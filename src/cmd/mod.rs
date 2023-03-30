use crate::api::DracoonBuilder;
use self::models::DcCmdError;

pub mod models;
pub mod credentials;

const CLIENT_ID: &str = "xxxxxxxxxxx";
const CLIENT_SECRET: &str = "xxxxxxxxxxx";

pub fn download(source: String, target: String) -> Result<(), DcCmdError>{

    let base_url = parse_base_url(source)?;

    let dracoon = DracoonBuilder::new()
                                 .with_base_url(base_url)
                                 .with_client_id(CLIENT_ID)
                                 .with_client_secret(CLIENT_SECRET)
                                 .build()?;

    Ok(())

}

pub fn upload(source: String, target: String) {

    todo!()

}

fn parse_base_url(url_str: String) -> Result<String, DcCmdError> {

if url_str.starts_with("http://") {
    return Err(DcCmdError::InvalidUrl)
};

let url_str = match url_str.starts_with("https://") {
    true => url_str,
    false => format!("https://{url_str}")
};

let uri_fragments: Vec<&str> = url_str[8..].split("/").collect();

match uri_fragments.len() {
    2.. => Ok(format!("https://{}", uri_fragments[0])),
    _ => Err(DcCmdError::InvalidUrl)
}
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_base_url_parse_https() {
        let base_url = parse_base_url("https://bla.dracoon.com/bla/somefile.pdf".into()).unwrap();
        assert_eq!(base_url, "https://bla.dracoon.com");
    }

    #[test]
    fn test_base_url_parse_no_https() {
        let base_url = parse_base_url("bla.dracoon.com/bla/somefile.pdf".into()).unwrap();
        assert_eq!(base_url, "https://bla.dracoon.com");
    }

    #[test]
    fn test_base_url_parse_invalid_path() {
        let base_url = parse_base_url("bla.dracoon.com".into());
        assert_eq!(base_url, Err(DcCmdError::InvalidUrl));
    }

}