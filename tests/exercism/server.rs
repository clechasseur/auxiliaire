pub(super) mod handler;

use std::collections::HashMap;
use std::sync::Arc;
use mini_exercism::api;
use mini_exercism::api::v2::solution::Solution;
use mini_exercism::core::Credentials;
use reqwest::Method;
use wiremock::{Mock, MockServer, ResponseTemplate};
use wiremock::matchers::{bearer_token, method, path};
use wiremock_logical_matchers::not;
use crate::exercism::server::handler::v2;

pub struct ExercismServer {
    api_token: String,
    mock_server: MockServer,
    pub(crate) solutions: HashMap<String, Solution>,
}

impl ExercismServer {
    pub async fn new<T>(api_token: T) -> Arc<Self>
    where
        T: Into<String>,
    {
        let api_token = api_token.into();
        let mock_server = Self::create_mock_server(&api_token).await;

        let server = Arc::new(Self {
            api_token,
            mock_server,
            solutions: HashMap::new(),
        });
        
        Self::install_v2_handlers(Arc::clone(&server)).await;
        
        server
    }

    pub fn api_base_url(&self) -> String {
        self.mock_server.uri()
    }

    pub fn v1_client(&self) -> api::v1::Client {
        api::v1::Client::builder()
            .api_base_url(&self.api_base_url())
            .credentials(Credentials::from_api_token(self.api_token.clone()))
            .build()
            .unwrap()
    }

    pub fn v2_client(&self) -> api::v2::Client {
        api::v2::Client::builder()
            .api_base_url(&self.api_base_url())
            .credentials(Credentials::from_api_token(self.api_token.clone()))
            .build()
            .unwrap()
    }

    pub fn add_solution(&mut self, solution: Solution) {
        if solution.track.name.is_empty() {
            panic!("cannot add solution without a track name");
        }

        self.solutions.insert(solution.track.name.clone(), solution);
    }

    pub fn add_solution_json<J>(&mut self, solution_json: J)
    where
        J: AsRef<str>,
    {
        let solution: Solution = serde_json::from_str(solution_json.as_ref()).unwrap();
        self.add_solution(solution);
    }

    async fn create_mock_server(api_token: &str) -> MockServer {
        let mock_server = MockServer::start().await;

        Mock::given(not(method(Method::OPTIONS)))
            .and(not(bearer_token(api_token)))
            .respond_with(ResponseTemplate::new(403))
            .mount(&mock_server)
            .await;

        mock_server
    }

    async fn install_v2_handlers(this: Arc<Self>) {
        Mock::given(method(Method::GET))
            .and(path("/v2/solutions"))
            .and(bearer_token(&this.api_token))
            .respond_with(v2::solutions::Handler::new(Arc::downgrade(&this)))
            .mount(&this.mock_server)
            .await;
    }
}
