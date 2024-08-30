use std::sync::{Arc, Weak};
use mini_exercism::api::v2::solution;
use wiremock::{Request, Respond, ResponseTemplate};
use auxiliaire::command::backup::args::SolutionStatus;
use crate::exercism::server::ExercismServer;

pub struct Handler {
    server: Weak<ExercismServer>,
}

impl Handler {
    pub fn new(server: Weak<ExercismServer>) -> Self {
        Self { server }
    }

    fn server(&self) -> Arc<ExercismServer> {
        self.server.upgrade().expect("server should be kept alive while requests are processed")
    }
}

impl Respond for Handler {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        // TODO
        ResponseTemplate::new(500)
    }
}

#[derive(Debug, Default)]
struct Filters {
    pub criteria: Option<String>,
    pub track: Option<String>,
    pub status: Option<SolutionStatus>,
}

impl From<&Request> for Filters {
    fn from(value: &Request) -> Self {
        let mut filters = Self::default();

        for (key, value) in value.url.query_pairs() {
            match key.as_ref() {
                "criteria" => filters.criteria = Some(value.into_owned()),
                "track" => filters.track = Some(value.into_owned()),
                "status" => {
                    let status: solution::Status = value.parse().unwrap();
                }
                _ => (),
            }
        }

        filters
    }
}
