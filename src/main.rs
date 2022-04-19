use fastly::http::{header, Method, StatusCode, HeaderValue, request};
use fastly::{mime, Error, Request, Response};
use std::collections::HashMap;
use std::io::Cursor;
use fastly::http::Url;

#[fastly::main]
fn main(req: Request) -> Result<Response, Error> {
    // Filter request methods...
    match req.get_method() {
        // Allow GET and HEAD requests.
        &Method::GET | &Method::HEAD => (),

        // Deny anything else.
        _ => {
            return Ok(Response::from_status(StatusCode::METHOD_NOT_ALLOWED)
                .with_header(header::ALLOW, "GET, HEAD")
                .with_body_text_plain("This method is not allowed\n"))
        }
    };

    // Pattern match on the path...
    match req.get_path() {
        "/" => {
            let parsed_url = Url::parse(req.get_path()).unwrap();
            let hash_query: HashMap<_, _> = parsed_url.query_pairs().into_owned().collect();

            let image_src = match hash_query.get("src") {
                Some(val) => val,
                None => return response_bad_request("Missing src"),
            };

            let image_width = match hash_query.get("w") {
                Some(val) => val.parse::<u32>().unwrap(),
                None => return response_bad_request("Missing width"),
            };

            let image_quality = match hash_query.get("q") {
                Some(val) => val.parse::<u8>().unwrap(),
                None => 80,
            };

            let supported_image_format = match req.get_header("Accept") {
                Some(h) => h,
                None => &HeaderValue::from_static("image/jpeg")
            };

            let load_image = Request::get(image_src).send_async("image");

            let pending_reqs = vec![load_image];

            let (resp, pending_reqs) = match request::select(pending_reqs)?;

            let mut image_resp = resp?;

            match image_resp.get_status() {
                StatusCode::OK => {
                    let image_to_bytes = image_resp.get_body_mut().into_bytes();

                    let image = match image::load_from_memory(&image_to_bytes) {
                        Ok(value) => value,
                        _ => return response_bad_request("Error loading image from memory")
                    };

                    let image_transform_format = image::ImageFormat::Jpeg;

                    let image_transform_format_header = "image/jpeg";

                    // if format!("{:?}", supported_image_format).contains("image/webp") {
                    //   image_transform_format = image::ImageFormat::WebP;
                    //   image_transform_format_header = "image/webp".to_string();
                    // }

                    // if format!("{:?}", supported_image_format).contains("image/avif") {
                    //   image_transform_format =  image::ImageFormat::Avif;
                    //   image_transform_format_header = "image/avif".to_string();
                    // }

                    let image = match image.resize(
                        image_width,
                        u32::MAX,
                        image::imageops::FilterType::Nearest,
                    ) {
                        // Yes, this is bad :)
                        data => data,
                        _ => return response_bad_request("Error when resizing image")
                    };

                    // Remove the type for `new_image` as it's inferred
                    let mut new_image =
                        Vec::with_capacity(image.width() as usize * image.height() as usize);

                    image
                        .write_to(&mut Cursor::new(&mut new_image), image_transform_format)
                        .expect("Error writing image");


                    Ok(
                        Response::from_status(StatusCode::OK)
                        .with_header("Content-Type", image_transform_format_header)
                    )
                },
                _ => return response_bad_request("Bad Request"),
            }
        },
        _ => Ok(
            Response::from_status(StatusCode::NOT_FOUND)
            .with_body_text_plain("The page you requested could not be found\n")
        ),
    };
}
