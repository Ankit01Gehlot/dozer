use std::path::Path;

use super::generator::{ProtoGenerator, ServiceDesc};
use crate::test_utils;
use dozer_cache::dozer_log::schemas::EndpointSchema;
use tempdir::TempDir;

fn read_service_desc(proto_folder_path: &Path, table_name: &str) -> ServiceDesc {
    let descriptor_path = proto_folder_path.join("descriptor.bin");
    ProtoGenerator::generate_descriptor(proto_folder_path, &descriptor_path, &[table_name])
        .unwrap();
    let descriptor_bytes = std::fs::read(&descriptor_path).unwrap();
    ProtoGenerator::read_schema(&descriptor_bytes, table_name).unwrap()
}

#[test]
fn test_generate_proto_and_descriptor() {
    let table_name = "films";
    let (schema, secondary_indexes) = test_utils::get_schema();
    let endpoint = test_utils::get_endpoint();

    let schema = EndpointSchema {
        path: endpoint.path,
        schema,
        secondary_indexes,
        enable_token: false,
        enable_on_event: false,
        connections: Default::default(),
    };

    let tmp_dir = TempDir::new("proto_generated").unwrap();
    let tmp_dir_path = tmp_dir.path();

    ProtoGenerator::generate(tmp_dir_path, table_name, &schema).unwrap();

    let service_desc = read_service_desc(tmp_dir_path, table_name);

    assert_eq!(
        service_desc
            .query
            .response_desc
            .record_desc
            .message
            .full_name(),
        "dozer.generated.films.Film"
    );
    assert!(service_desc.token.is_none());
}

#[test]
fn test_generate_proto_and_descriptor_with_security() {
    let table_name = "films";
    let (schema, secondary_indexes) = test_utils::get_schema();
    let endpoint = test_utils::get_endpoint();

    let schema = EndpointSchema {
        path: endpoint.path,
        schema,
        secondary_indexes,
        enable_token: true,
        enable_on_event: true,
        connections: Default::default(),
    };

    let tmp_dir = TempDir::new("proto_generated").unwrap();
    let tmp_dir_path = tmp_dir.path();

    ProtoGenerator::generate(tmp_dir_path, table_name, &schema).unwrap();

    let service_desc = read_service_desc(tmp_dir_path, table_name);

    assert_eq!(
        service_desc
            .query
            .response_desc
            .record_desc
            .message
            .full_name(),
        "dozer.generated.films.Film"
    );
    assert_eq!(
        service_desc
            .token
            .unwrap()
            .response_desc
            .message
            .full_name(),
        "dozer.generated.films.TokenResponse"
    );
}

#[test]
fn test_generate_proto_and_descriptor_with_push_event_off() {
    let table_name = "films";
    let (schema, secondary_indexes) = test_utils::get_schema();
    let endpoint = test_utils::get_endpoint();

    let schema = EndpointSchema {
        path: endpoint.path,
        schema,
        secondary_indexes,
        enable_token: true,
        enable_on_event: false,
        connections: Default::default(),
    };

    let tmp_dir = TempDir::new("proto_generated").unwrap();
    let tmp_dir_path = tmp_dir.path();
    ProtoGenerator::generate(tmp_dir_path, table_name, &schema).unwrap();

    let service_desc = read_service_desc(tmp_dir_path, table_name);

    assert_eq!(
        service_desc
            .query
            .response_desc
            .record_desc
            .message
            .full_name(),
        "dozer.generated.films.Film"
    );
    assert_eq!(
        service_desc
            .token
            .unwrap()
            .response_desc
            .message
            .full_name(),
        "dozer.generated.films.TokenResponse"
    );
    assert!(service_desc.on_event.is_none());
}
