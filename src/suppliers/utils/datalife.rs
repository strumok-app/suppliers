
use reqwest::{self, RequestBuilder};

pub fn search_request(url: &str, query: &String) -> RequestBuilder {
    let client =  super::create_client();

    client.post(format!("{}/index.php", url))
        .form(&[
            ("do", "search"),
            ("subaction", "search"),
            ("story", &query),
            ("sortby", "date"),
            ("resorder", "desc")
        ])
}