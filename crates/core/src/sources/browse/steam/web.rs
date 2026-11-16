use super::{Failure, Transport};
use crate::rimworld::steamapi::{ApiKey, Call};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};

/// Where the key goes. Never the address, so no address that reaches an error or a log has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carry {
    /// `x-webapi-key`, which only a community page documents, not Valve.
    Header,
    /// A POST form with `key` and `input_json`.
    Form,
}

pub trait Wire: Send + Sync {
    fn send(&self, url: &str, carry: Carry, args: &str, key: &str) -> Result<String, Failure>;
}

pub struct Http {
    agent: ureq::Agent,
}

impl Default for Http {
    fn default() -> Self {
        Http {
            agent: crate::base::web::steam_agent(),
        }
    }
}

impl Wire for Http {
    fn send(&self, url: &str, carry: Carry, args: &str, key: &str) -> Result<String, Failure> {
        let sent = match carry {
            Carry::Header => self
                .agent
                .get(url)
                .query("format", "json")
                .query("input_json", args)
                .header("x-webapi-key", key)
                .call(),
            Carry::Form => self
                .agent
                .post(url)
                .query("format", "json")
                .send_form([("key", key), ("input_json", args)]),
        };
        let mut response = sent.map_err(|e| match e {
            ureq::Error::StatusCode(code) => Failure::Status(code),
            _ => Failure::Unreachable,
        })?;
        crate::base::web::read_text(&mut response, 64 * 1024 * 1024)
            .map_err(|_| Failure::Unreachable)
    }
}

/// Sends the key in a header; when Steam refuses that with 401 or 403, once more in a form body,
/// and keeps to the body for the rest of the session if that worked.
pub struct Web<W = Http> {
    wire: W,
    form: AtomicBool,
}

impl Default for Web {
    fn default() -> Self {
        Web::over(Http::default())
    }
}

impl<W: Wire> Web<W> {
    pub fn over(wire: W) -> Self {
        Web {
            wire,
            form: AtomicBool::new(false),
        }
    }
}

fn refused(failure: Failure) -> bool {
    matches!(failure, Failure::Status(401 | 403))
}

impl<W: Wire> Transport for Web<W> {
    fn call(&self, call: Call, args: &Value, key: &ApiKey) -> Result<String, Failure> {
        let (url, args) = (call.url(), args.to_string());
        let send = |carry| self.wire.send(&url, carry, &args, key.reveal());
        if self.form.load(Ordering::Relaxed) {
            return send(Carry::Form);
        }
        match send(Carry::Header) {
            Err(failure) if refused(failure) => {
                let answer = send(Carry::Form)?;
                self.form.store(true, Ordering::Relaxed);
                Ok(answer)
            }
            answer => answer,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct Fake {
        accepts: &'static [Carry],
        refuses_with: u16,
        sent: Mutex<Vec<Carry>>,
    }

    impl Fake {
        fn accepting(accepts: &'static [Carry]) -> Self {
            Fake {
                accepts,
                refuses_with: 403,
                sent: Mutex::default(),
            }
        }

        fn sent(&self) -> Vec<Carry> {
            self.sent.lock().unwrap().clone()
        }
    }

    impl Wire for &Fake {
        fn send(&self, _: &str, carry: Carry, _: &str, _: &str) -> Result<String, Failure> {
            self.sent.lock().unwrap().push(carry);
            match self.accepts.contains(&carry) {
                true => Ok("{}".into()),
                false => Err(Failure::Status(self.refuses_with)),
            }
        }
    }

    fn ask(web: &Web<&Fake>) -> Result<String, Failure> {
        let key = ApiKey::new("SECRETKEY").unwrap();
        web.call(Call::QueryFiles, &serde_json::json!({}), &key)
    }

    #[test]
    fn the_header_is_tried_first_and_kept_when_it_works() {
        let fake = Fake::accepting(&[Carry::Header]);
        let web = Web::over(&fake);
        ask(&web).unwrap();
        ask(&web).unwrap();
        assert_eq!(fake.sent(), [Carry::Header, Carry::Header]);
    }

    #[test]
    fn a_header_steam_refuses_is_retried_in_a_form_and_the_form_is_remembered() {
        let fake = Fake::accepting(&[Carry::Form]);
        let web = Web::over(&fake);
        ask(&web).unwrap();
        ask(&web).unwrap();
        assert_eq!(fake.sent(), [Carry::Header, Carry::Form, Carry::Form]);
    }

    #[test]
    fn a_refusal_in_both_ways_is_a_refusal_and_nothing_is_remembered() {
        for code in [401, 403] {
            let mut fake = Fake::accepting(&[]);
            fake.refuses_with = code;
            let web = Web::over(&fake);
            assert_eq!(ask(&web), Err(Failure::Status(code)));
            assert_eq!(ask(&web), Err(Failure::Status(code)));
            let header_then_form = [Carry::Header, Carry::Form, Carry::Header, Carry::Form];
            assert_eq!(fake.sent(), header_then_form);
        }
    }

    #[test]
    fn another_failure_is_not_retried() {
        struct Down(Mutex<usize>);
        impl Wire for &Down {
            fn send(&self, _: &str, _: Carry, _: &str, _: &str) -> Result<String, Failure> {
                *self.0.lock().unwrap() += 1;
                Err(Failure::Status(500))
            }
        }
        let down = Down(Mutex::new(0));
        let key = ApiKey::new("SECRETKEY").unwrap();
        let answer = Web::over(&down).call(Call::GetDetails, &Value::Null, &key);
        assert_eq!(answer, Err(Failure::Status(500)));
        assert_eq!(*down.0.lock().unwrap(), 1);
    }
}
