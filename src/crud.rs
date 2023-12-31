use bytes::BufMut;
use futures::TryStreamExt;
use rusqlite::Connection;
use serde_json::Value;
use std::collections::HashMap;
use warp::filters::multipart::{FormData, Part};
use warp::reject::{self, InvalidQuery, Rejection};
use warp::reply::Reply;
use warp::{reject::reject, Filter};

use crate::middleware::{check_file_type, with_auth, MyError};
use crate::user_db_managment;
use crate::validations::{DatabaseError, Errors, MissingParameter, NoMatchingUser};
use crate::{author_db_managment, post_db_managment, validations::Validations};
pub fn author_filter() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let author_base = warp::path("authors");

    let get = warp::get()
        .and(author_base)
        .and(warp::path::param::<i16>())
        .and(warp::path::end())
        .and_then(get_author);

    let create = warp::post()
        .and(author_base)
        .and(with_auth().untuple_one())
        .and(warp::query::<HashMap<String, String>>())
        .and(warp::path::end())
        .and_then(create_author);

    let list = warp::get()
        .and(author_base)
        .and(warp::path::end())
        .and_then(list_authors);

    create.or(get).or(list)
}

async fn list_authors() -> Result<impl warp::Reply, warp::Rejection> {
    match author_db_managment::list_authors().await {
        Ok(reply) => Ok(reply),
        Err(_) => Err(warp::reject::custom(DatabaseError)),
    }
}

async fn get_author(id: i16) -> Result<impl warp::Reply, warp::Rejection> {
    match author_db_managment::get_author(id).await {
        Ok(reply) => Ok(reply),
        Err(_) => Err(warp::reject::custom(DatabaseError)),
    }
}

async fn create_author(
    param: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    if let Some(name) = param.get("name") {
        match Validations::validate(name) {
            Ok(_) => match author_db_managment::create_author(name).await {
                Ok(reply) => Ok(reply),
                Err(_) => Err(warp::reject::custom(DatabaseError)),
            },
            Err(rejection) => Err(rejection),
        }
    } else {
        // Handle the case where "name" is missing in the HashMap.
        Err(warp::reject::custom(MissingParameter))
    }
}

pub fn post_filter() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let post_base = warp::path("posts");

    let get = warp::get()
        .and(post_base)
        .and(warp::path::param::<i16>())
        .and(warp::path::end())
        .and_then(get_post);

    let list = warp::get()
        .and(post_base)
        .and(warp::path::end())
        .and_then(list_posts);

    let create = warp::post()
        .and(post_base)
        .and(with_auth().untuple_one())
        .and(warp::query::<HashMap<String, String>>())
        .and(warp::body::json())
        .and(warp::path::end())
        .and_then(create_post);

    let delete = warp::post()
        .and(post_base)
        .and(warp::path("delete"))
        .and(with_auth().untuple_one())
        .and(warp::path::param::<i16>())
        .and(warp::path::end())
        .and_then(delete_post);

    let upload = warp::post()
        .and(post_base)
        .and(warp::path("upload"))
        .and(warp::path::param::<i16>())
        // .and(check_file_type().untuple_one())
        .and(with_auth().untuple_one())
        .and(warp::body::json())
        .and_then(upload_files);

    let download = warp::post()
        .and(post_base)
        .and(warp::path("download"))
        .and(warp::query::<HashMap<String, String>>())
        .and(warp::path::end())
        // .and(check_file_type().untuple_one())
        .and(with_auth().untuple_one())
        .and_then(download_files);

    upload.or(create).or(get).or(list).or(delete).or(download)
}

async fn list_posts() -> Result<impl warp::Reply, warp::Rejection> {
    match post_db_managment::list_posts().await {
        Ok(reply) => Ok(reply),
        Err(_) => Err(warp::reject::custom(DatabaseError)),
    }
}

async fn get_post(id: i16) -> Result<impl warp::Reply, warp::Rejection> {
    match post_db_managment::get_post(id).await {
        Ok(reply) => Ok(reply),
        Err(_) => Err(warp::reject::custom(DatabaseError)),
    }
}

async fn create_post(
    param: HashMap<String, String>,
    files: Value,
) -> Result<impl warp::Reply, warp::Rejection> {
    if let (Some(title), Some(body), Some(author_id)) = (
        param.get("title"),
        param.get("body"),
        param.get("author_id"),
    ) {
        match Validations::validate(title) {
            Ok(_) => match post_db_managment::create_post(title, body, author_id, files).await {
                Ok(reply) => Ok(reply),
                Err(_) => Err(reject()),
            },
            Err(rejection) => Err(rejection),
        }
    } else {
        Err(warp::reject::custom(Errors::InvalidQuery))
    }
}

async fn delete_post(id: i16) -> Result<impl warp::Reply, warp::Rejection> {
    match post_db_managment::delete_post(id).await {
        Ok(reply) => Ok(reply),
        Err(_) => Err(warp::reject::custom(DatabaseError)),
    }
}

async fn upload_files(post_id: i16, data: Value) -> Result<impl Reply, Rejection> {
    match post_db_managment::upload_files(data, &post_id.to_string(), None).await {
        Ok(reply) => Ok(reply),
        Err(rejection) => Err(rejection),
    }
}

async fn download_files(param: HashMap<String, String>) -> Result<impl Reply, Rejection> {
    if let (Some(post_id), Some(file_name)) = (param.get("post_id"), param.get("file_name")) {
        match post_db_managment::download_files(post_id, file_name).await {
            Ok(reply) => Ok(reply),
            Err(rejection) => Err(rejection),
        }
    } else {
        return Err(warp::reject::custom(Errors::InvalidQuery));
    }
}

pub fn user_filter() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let user_filter_base = warp::path("users");
    let create = warp::post()
        .and(user_filter_base)
        .and(warp::query::<HashMap<String, String>>())
        .and(warp::path::end())
        .and_then(create_user);

    let login = warp::get()
        .and(user_filter_base)
        .and(warp::path("login"))
        .and(warp::query::<HashMap<String, String>>())
        .and(warp::path::end())
        .and_then(login_user);

    return create.or(login);
}

async fn login_user(param: HashMap<String, String>) -> Result<impl warp::Reply, warp::Rejection> {
    if let (Some(username), Some(password)) = (param.get("username"), param.get("password")) {
        match validate_all_params(vec![username.to_string(), password.to_string()]) {
            Ok(_) => match user_db_managment::login_user(username, password).await {
                Ok((reply, token)) => {
                    // Create a cookie containing the JWT token
                    let cookie_value = format!("jwt={}; HttpOnly", token);

                    // Convert the cookie value to a HeaderValue
                    let cookie_header_value =
                        warp::http::header::HeaderValue::from_str(&cookie_value)
                            .expect("Failed to create HeaderValue from cookie value");

                    // Set the cookie in the response headers
                    let response = warp::reply::with_header(
                        reply,
                        warp::http::header::SET_COOKIE,
                        cookie_header_value,
                    );
                    Ok(response)
                }
                Err(e) => {
                    if e == rusqlite::Error::InvalidQuery {
                        Err(warp::reject::custom(NoMatchingUser))
                    } else {
                        Err(warp::reject::custom(DatabaseError))
                    }
                }
            },
            Err(rejection) => Err(rejection),
        }
    } else {
        Err(warp::reject::custom(MissingParameter))
    }
}

async fn create_user(param: HashMap<String, String>) -> Result<impl warp::Reply, warp::Rejection> {
    if let (Some(username), Some(password)) = (param.get("username"), param.get("password")) {
        match validate_all_params(vec![username.to_string(), password.to_string()]) {
            Ok(_) => match user_db_managment::create_user(username, password).await {
                Ok(reply) => Ok(reply),
                Err(_) => Err(warp::reject::custom(DatabaseError)),
            },
            Err(rejection) => Err(rejection),
        }
    } else {
        Err(warp::reject::custom(MissingParameter))
    }
}

fn validate_all_params(params: Vec<String>) -> Result<bool, Rejection> {
    for param in params.iter() {
        match Validations::validate(&param) {
            Ok(()) => continue,
            Err(rejection) => return Err(rejection),
        }
    }
    Ok(true)
}
