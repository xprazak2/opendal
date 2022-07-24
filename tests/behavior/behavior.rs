// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! `behavior` intents to provide behavior tests for all storage services.
//!
//! # Note
//!
//! `behavior` requires most of the logic is correct, especially `write` and `delete`. We will not depends service specific functions to prepare the fixtures.
//!
//! For examples, we depends `write` to create a file before testing `read`. If `write` doesn't works well, we can't test `read` correctly too.

use std::collections::HashMap;
use std::io;

use anyhow::Result;
use futures::TryStreamExt;
use log::debug;
use opendal::services;
use opendal::ObjectMode;
use opendal::Operator;
use sha2::Digest;
use sha2::Sha256;

use super::init_logger;
use super::utils::*;

/// Generate real test cases.
/// Update function list while changed.
macro_rules! behavior_tests {
    ($($service:ident),*) => {
        $(
            behavior_test!(
                $service,

                test_check,
                test_metadata,
                test_object_path,
                test_object_id,
                test_object_name,

                test_create_file,
                test_create_file_existing,
                test_create_dir,
                test_create_dir_exising,

                test_write,
                test_write_with_dir_path,

                test_read_full,
                test_read_range,
                test_read_not_exist,
                test_read_with_dir_path,
                #[cfg(feature = "compress")]
                test_read_decompress_gzip,
                #[cfg(feature = "compress")]
                test_read_decompress_gzip_with,
                #[cfg(feature = "compress")]
                test_read_decompress_zstd,
                #[cfg(feature = "compress")]
                test_read_decompress_zstd_with,

                test_stat,
                test_stat_dir,
                test_stat_not_cleaned_path,
                test_stat_not_exist,
                test_stat_root,

                test_list_dir,
                test_list_sub_dir,
                test_list_dir_with_file_path,
                test_list_nested_dir,
                test_list_empty_dir,

                test_delete,
                test_delete_not_existing,
                test_delete_empty_dir,

                test_walk_bottom_up,
                test_walk_top_down,
                test_remove_all,
            );
        )*
    };
}

macro_rules! behavior_test {
    ($service:ident, $($(#[$meta:meta])* $test:ident),*,) => {
        paste::item! {
            mod [<$service>] {
                use super::*;

                $(
                    #[tokio::test]
                    $(
                        #[$meta]
                    )*
                    async fn [< $test >]() -> Result<()> {
                        init_logger();
                        let _ = dotenv::dotenv();

                        let acc = super::services::$service::tests::new().await?;
                        if acc.is_none() {
                            return Ok(())
                        }
                        super::$test(Operator::new(acc.unwrap())).await
                    }
                )*
            }
        }
    };
}

behavior_tests!(azblob, fs, memory, s3, ipfs);

cfg_if::cfg_if! {
    if #[cfg(feature = "services-hdfs")] {
        behavior_tests!(hdfs);
    }
}

/// Create file with file path should succeed.
async fn test_check(op: Operator) -> Result<()> {
    op.check(".opendal").await.expect("operator check is ok");

    Ok(())
}

/// Create file with file path should succeed.
async fn test_metadata(op: Operator) -> Result<()> {
    let _ = op.metadata();

    Ok(())
}

/// Test object id.
async fn test_object_id(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();

    let o = op.object(&path);

    assert_eq!(o.id(), format!("{}{}", op.metadata().root(), path));

    Ok(())
}

/// Test object path.
async fn test_object_path(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();

    let o = op.object(&path);

    assert_eq!(o.path(), path);

    Ok(())
}

/// Test object name.
async fn test_object_name(op: Operator) -> Result<()> {
    // Normal
    let path = uuid::Uuid::new_v4().to_string();

    let o = op.object(&path);
    assert_eq!(o.name(), path);

    // File in subdir
    let name = uuid::Uuid::new_v4().to_string();
    let path = format!("{}/{}", uuid::Uuid::new_v4(), name);

    let o = op.object(&path);
    assert_eq!(o.name(), name);

    // Dir in subdir
    let name = uuid::Uuid::new_v4().to_string();
    let path = format!("{}/{}/", uuid::Uuid::new_v4(), name);

    let o = op.object(&path);
    assert_eq!(o.name(), format!("{name}/"));

    Ok(())
}

/// Create file with file path should succeed.
async fn test_create_file(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();

    let o = op.object(&path);

    let _ = o.create().await?;

    let meta = o.metadata().await?;
    assert_eq!(meta.mode(), ObjectMode::FILE);
    assert_eq!(meta.content_length(), 0);

    op.object(&path)
        .delete()
        .await
        .expect("delete must succeed");
    Ok(())
}

/// Create file on existing file path should succeed.
async fn test_create_file_existing(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();

    let o = op.object(&path);

    let _ = o.create().await?;

    let _ = o.create().await?;

    let meta = o.metadata().await?;
    assert_eq!(meta.mode(), ObjectMode::FILE);
    assert_eq!(meta.content_length(), 0);

    op.object(&path)
        .delete()
        .await
        .expect("delete must succeed");
    Ok(())
}

/// Create dir with dir path should succeed.
async fn test_create_dir(op: Operator) -> Result<()> {
    let path = format!("{}/", uuid::Uuid::new_v4());

    let o = op.object(&path);

    let _ = o.create().await?;

    let meta = o.metadata().await?;
    assert_eq!(meta.mode(), ObjectMode::DIR);

    op.object(&path)
        .delete()
        .await
        .expect("delete must succeed");
    Ok(())
}

/// Create dir on existing dir should succeed.
async fn test_create_dir_exising(op: Operator) -> Result<()> {
    let path = format!("{}/", uuid::Uuid::new_v4());

    let o = op.object(&path);

    let _ = o.create().await?;

    let _ = o.create().await?;

    let meta = o.metadata().await?;
    assert_eq!(meta.mode(), ObjectMode::DIR);

    op.object(&path)
        .delete()
        .await
        .expect("delete must succeed");
    Ok(())
}

/// Write a single file and test with stat.
async fn test_write(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();
    debug!("Generate a random file: {}", &path);
    let (content, size) = gen_bytes();

    let _ = op.object(&path).write(&content).await?;

    let meta = op
        .object(&path)
        .metadata()
        .await
        .expect("stat must succeed");
    assert_eq!(meta.content_length(), size as u64);

    op.object(&path)
        .delete()
        .await
        .expect("delete must succeed");
    Ok(())
}

/// Write file with dir path should return an error
async fn test_write_with_dir_path(op: Operator) -> Result<()> {
    let path = format!("{}/", uuid::Uuid::new_v4());
    let (content, _) = gen_bytes();

    let result = op.object(&path).write(&content).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Is a directory"));

    Ok(())
}

/// Stat existing file should return metadata
async fn test_stat(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();
    debug!("Generate a random file: {}", &path);
    let (content, size) = gen_bytes();

    let _ = op
        .object(&path)
        .write(&content)
        .await
        .expect("write must succeed");

    let meta = op.object(&path).metadata().await?;
    assert_eq!(meta.mode(), ObjectMode::FILE);
    assert_eq!(meta.content_length(), size as u64);

    op.object(&path)
        .delete()
        .await
        .expect("delete must succeed");
    Ok(())
}

/// Stat existing file should return metadata
async fn test_stat_dir(op: Operator) -> Result<()> {
    let path = format!("{}/", uuid::Uuid::new_v4());

    let _ = op.object(&path).create().await.expect("write must succeed");

    let meta = op.object(&path).metadata().await?;
    assert_eq!(meta.mode(), ObjectMode::DIR);

    op.object(&path)
        .delete()
        .await
        .expect("delete must succeed");
    Ok(())
}

/// Stat not cleaned path should also succeed.
async fn test_stat_not_cleaned_path(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();
    debug!("Generate a random file: {}", &path);
    let (content, size) = gen_bytes();

    let _ = op
        .object(&path)
        .write(&content)
        .await
        .expect("write must succeed");

    let meta = op.object(&format!("//{}", &path)).metadata().await?;
    assert_eq!(meta.mode(), ObjectMode::FILE);
    assert_eq!(meta.content_length(), size as u64);

    op.object(&path)
        .delete()
        .await
        .expect("delete must succeed");
    Ok(())
}

/// Stat not exist file should return NotFound
async fn test_stat_not_exist(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();

    let meta = op.object(&path).metadata().await;
    assert!(meta.is_err());
    assert_eq!(meta.unwrap_err().kind(), io::ErrorKind::NotFound);

    Ok(())
}

/// Root should be able to stat and returns DIR.
async fn test_stat_root(op: Operator) -> Result<()> {
    let meta = op.object("").metadata().await?;
    assert_eq!(meta.mode(), ObjectMode::DIR);

    let meta = op.object("/").metadata().await?;
    assert_eq!(meta.mode(), ObjectMode::DIR);

    Ok(())
}

/// Read full content should match.
async fn test_read_full(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();
    debug!("Generate a random file: {}", &path);
    let (content, size) = gen_bytes();

    let _ = op
        .object(&path)
        .write(&content)
        .await
        .expect("write must succeed");

    let bs = op.object(&path).read().await?;
    assert_eq!(size, bs.len(), "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        format!("{:x}", Sha256::digest(&content)),
        "read content"
    );

    op.object(&path)
        .delete()
        .await
        .expect("delete must succeed");
    Ok(())
}

/// Read range content should match.
async fn test_read_range(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();
    debug!("Generate a random file: {}", &path);
    let (content, size) = gen_bytes();
    let (offset, length) = gen_offset_length(size as usize);

    let _ = op
        .object(&path)
        .write(&content)
        .await
        .expect("write must succeed");

    let bs = op.object(&path).range_read(offset..offset + length).await?;
    assert_eq!(bs.len() as u64, length, "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        format!(
            "{:x}",
            Sha256::digest(&content[offset as usize..(offset + length) as usize])
        ),
        "read content"
    );

    op.object(&path)
        .delete()
        .await
        .expect("delete must succeed");
    Ok(())
}

/// Read not exist file should return NotFound
async fn test_read_not_exist(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();

    let bs = op.object(&path).read().await;
    assert!(bs.is_err());
    assert_eq!(bs.unwrap_err().kind(), io::ErrorKind::NotFound);

    Ok(())
}

/// Read with dir path should return an error.
async fn test_read_with_dir_path(op: Operator) -> Result<()> {
    let path = format!("{}/", uuid::Uuid::new_v4());

    let _ = op.object(&path).create().await.expect("write must succeed");

    let result = op.object(&path).read().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Is a directory"));

    op.object(&path)
        .delete()
        .await
        .expect("delete must succeed");
    Ok(())
}

// Read a compressed gzip file.
#[cfg(feature = "compress")]
async fn test_read_decompress_gzip(op: Operator) -> Result<()> {
    use async_compression::futures::write::GzipEncoder;
    use futures::AsyncWriteExt;

    let path = format!("{}.gz", uuid::Uuid::new_v4());
    debug!("Generate a random file: {}", &path);
    let (content, size) = gen_bytes();

    let mut encoder = GzipEncoder::new(vec![]);
    encoder.write_all(&content).await?;
    encoder.close().await?;
    let compressed_content = encoder.into_inner();

    let _ = op.object(&path).write(&compressed_content).await?;

    let bs = op
        .object(&path)
        .decompress_read()
        .await?
        .expect("decompress read must succeed");
    assert_eq!(bs.len(), size, "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        format!("{:x}", Sha256::digest(&content)),
        "read content"
    );

    op.object(&path)
        .delete()
        .await
        .expect("delete must succeed");
    Ok(())
}

// Read a compressed gzip file with algo.
#[cfg(feature = "compress")]
async fn test_read_decompress_gzip_with(op: Operator) -> Result<()> {
    use async_compression::futures::write::GzipEncoder;
    use futures::AsyncWriteExt;
    use opendal::io_util::CompressAlgorithm;

    let path = format!("{}.gz", uuid::Uuid::new_v4());
    debug!("Generate a random file: {}", &path);
    let (content, size) = gen_bytes();

    let mut encoder = GzipEncoder::new(vec![]);
    encoder.write_all(&content).await?;
    encoder.close().await?;
    let compressed_content = encoder.into_inner();

    let _ = op.object(&path).write(&compressed_content).await?;

    let bs = op
        .object(&path)
        .decompress_read_with(CompressAlgorithm::Gzip)
        .await?;
    assert_eq!(bs.len(), size, "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        format!("{:x}", Sha256::digest(&content)),
        "read content"
    );

    op.object(&path)
        .delete()
        .await
        .expect("delete must succeed");
    Ok(())
}

// Read a compressed zstd file.
#[cfg(feature = "compress")]
async fn test_read_decompress_zstd(op: Operator) -> Result<()> {
    use async_compression::futures::write::ZstdEncoder;
    use futures::AsyncWriteExt;

    let path = format!("{}.zst", uuid::Uuid::new_v4());
    debug!("Generate a random file: {}", &path);
    let (content, size) = gen_bytes();

    let mut encoder = ZstdEncoder::new(vec![]);
    encoder.write_all(&content).await?;
    encoder.close().await?;
    let compressed_content = encoder.into_inner();

    let _ = op.object(&path).write(&compressed_content).await?;

    let bs = op
        .object(&path)
        .decompress_read()
        .await?
        .expect("decompress read must succeed");
    assert_eq!(bs.len(), size, "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        format!("{:x}", Sha256::digest(&content)),
        "read content"
    );

    op.object(&path)
        .delete()
        .await
        .expect("delete must succeed");
    Ok(())
}

// Read a compressed zstd file with algo.
#[cfg(feature = "compress")]
async fn test_read_decompress_zstd_with(op: Operator) -> Result<()> {
    use async_compression::futures::write::ZstdEncoder;
    use futures::AsyncWriteExt;
    use opendal::io_util::CompressAlgorithm;

    let path = format!("{}.zst", uuid::Uuid::new_v4());
    debug!("Generate a random file: {}", &path);
    let (content, size) = gen_bytes();

    let mut encoder = ZstdEncoder::new(vec![]);
    encoder.write_all(&content).await?;
    encoder.close().await?;
    let compressed_content = encoder.into_inner();

    let _ = op.object(&path).write(&compressed_content).await?;

    let bs = op
        .object(&path)
        .decompress_read_with(CompressAlgorithm::Zstd)
        .await?;
    assert_eq!(bs.len(), size, "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        format!("{:x}", Sha256::digest(&content)),
        "read content"
    );

    Ok(())
}

/// List dir should return newly created file.
async fn test_list_dir(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();
    debug!("Generate a random file: {}", &path);
    let (content, size) = gen_bytes();

    let _ = op
        .object(&path)
        .write(&content)
        .await
        .expect("write must succeed");

    let mut obs = op.object("/").list().await?;
    let mut found = false;
    while let Some(de) = obs.try_next().await? {
        let meta = de.metadata().await?;
        if de.path() == path {
            assert_eq!(meta.mode(), ObjectMode::FILE);
            assert_eq!(meta.content_length(), size as u64);

            found = true
        }
    }
    assert!(found, "file should be found in list");

    op.object(&path)
        .delete()
        .await
        .expect("delete must succeed");
    Ok(())
}

/// List empty dir should return nothing.
async fn test_list_empty_dir(op: Operator) -> Result<()> {
    let dir = format!("{}/", uuid::Uuid::new_v4());

    let _ = op.object(&dir).create().await.expect("write must succeed");

    let mut obs = op.object(&dir).list().await?;
    let mut objects = HashMap::new();
    while let Some(de) = obs.try_next().await? {
        objects.insert(de.path().to_string(), de);
    }
    debug!("got objects: {:?}", objects);

    assert_eq!(objects.len(), 0, "dir should only return empty");

    op.object(&dir).delete().await.expect("delete must succeed");
    Ok(())
}

/// List dir should return correct sub dir.
async fn test_list_sub_dir(op: Operator) -> Result<()> {
    let path = format!("{}/", uuid::Uuid::new_v4());

    let _ = op.object(&path).create().await.expect("creat must succeed");

    let mut obs = op.object("/").list().await?;
    let mut found = false;
    while let Some(de) = obs.try_next().await? {
        if de.path() == path {
            assert_eq!(de.mode(), ObjectMode::DIR);
            assert_eq!(de.name(), path);

            found = true
        }
    }
    assert!(found, "dir should be found in list");

    op.object(&path)
        .delete()
        .await
        .expect("delete must succeed");
    Ok(())
}

/// List dir should also to list nested dir.
async fn test_list_nested_dir(op: Operator) -> Result<()> {
    let dir = format!("{}/{}/", uuid::Uuid::new_v4(), uuid::Uuid::new_v4());

    let file_name = uuid::Uuid::new_v4().to_string();
    let file_path = format!("{dir}{file_name}");
    let dir_name = format!("{}/", uuid::Uuid::new_v4());
    let dir_path = format!("{dir}{dir_name}");

    let _ = op.object(&dir).create().await.expect("creat must succeed");
    let _ = op
        .object(&file_path)
        .create()
        .await
        .expect("creat must succeed");
    let _ = op
        .object(&dir_path)
        .create()
        .await
        .expect("creat must succeed");

    let mut obs = op.object(&dir).list().await?;
    let mut objects = HashMap::new();

    while let Some(de) = obs.try_next().await? {
        objects.insert(de.path().to_string(), de);
    }
    debug!("got objects: {:?}", objects);

    assert_eq!(objects.len(), 2, "dir should only got 2 objects");

    // Check file
    let meta = objects
        .get(&file_path)
        .expect("file should be found in list")
        .metadata()
        .await?;
    assert_eq!(meta.mode(), ObjectMode::FILE);
    assert_eq!(meta.content_length(), 0);

    // Check dir
    let meta = objects
        .get(&dir_path)
        .expect("file should be found in list")
        .metadata()
        .await?;
    assert_eq!(meta.mode(), ObjectMode::DIR);

    op.object(&file_path)
        .delete()
        .await
        .expect("delete must succeed");
    op.object(&dir_path)
        .delete()
        .await
        .expect("delete must succeed");
    op.object(&dir).delete().await.expect("delete must succeed");
    Ok(())
}

/// List with path file should auto add / suffix.
async fn test_list_dir_with_file_path(op: Operator) -> Result<()> {
    let parent = uuid::Uuid::new_v4().to_string();

    let obs = op.object(&parent).list().await.map(|_| ());
    assert!(obs.is_err());
    assert!(obs.unwrap_err().to_string().contains("Not a directory"));

    Ok(())
}

// Delete existing file should succeed.
async fn test_delete(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();
    debug!("Generate a random file: {}", &path);
    let (content, _) = gen_bytes();

    let _ = op
        .object(&path)
        .write(&content)
        .await
        .expect("write must succeed");

    let _ = op.object(&path).delete().await?;

    // Stat it again to check.
    assert!(!op.object(&path).is_exist().await?);

    Ok(())
}

// Delete empty dir should succeed.
async fn test_delete_empty_dir(op: Operator) -> Result<()> {
    let path = format!("{}/", uuid::Uuid::new_v4());

    let _ = op
        .object(&path)
        .create()
        .await
        .expect("create must succeed");

    let _ = op.object(&path).delete().await?;

    Ok(())
}

// Delete not existing file should also succeed.
async fn test_delete_not_existing(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();

    let _ = op.object(&path).delete().await?;

    Ok(())
}

// Walk top down should output as expected
async fn test_walk_top_down(op: Operator) -> Result<()> {
    let mut expected = vec![
        "x/", "x/y", "x/x/", "x/x/y", "x/x/x/", "x/x/x/y", "x/x/x/x/",
    ];
    for path in expected.iter() {
        op.object(path).create().await?;
    }

    let w = op.batch().walk_top_down("x/")?;
    let mut actual = w
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .map(|v| v.path().to_string())
        .collect::<Vec<_>>();

    debug!("walk top down: {:?}", actual);

    fn get_position(vs: &[String], s: &str) -> usize {
        vs.iter()
            .position(|v| v == s)
            .expect("{s} is not found in {vs}")
    }

    assert!(get_position(&actual, "x/x/x/x/") > get_position(&actual, "x/x/x/"));
    assert!(get_position(&actual, "x/x/x/") > get_position(&actual, "x/x/"));
    assert!(get_position(&actual, "x/x/") > get_position(&actual, "x/"));

    expected.sort_unstable();
    actual.sort_unstable();
    assert_eq!(actual, expected);
    Ok(())
}

// Walk bottom up should output as expected
async fn test_walk_bottom_up(op: Operator) -> Result<()> {
    let mut expected = vec![
        "x/", "x/y", "x/x/", "x/x/y", "x/x/x/", "x/x/x/y", "x/x/x/x/",
    ];
    for path in expected.iter() {
        op.object(path).create().await?;
    }

    let w = op.batch().walk_bottom_up("x/")?;
    let mut actual = w
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .map(|v| v.path().to_string())
        .collect::<Vec<_>>();

    debug!("walk bottom up: {:?}", actual);

    fn get_position(vs: &[String], s: &str) -> usize {
        vs.iter()
            .position(|v| v == s)
            .expect("{s} is not found in {vs}")
    }

    assert!(get_position(&actual, "x/x/x/x/") < get_position(&actual, "x/x/x/"));
    assert!(get_position(&actual, "x/x/x/") < get_position(&actual, "x/x/"));
    assert!(get_position(&actual, "x/x/") < get_position(&actual, "x/"));

    expected.sort_unstable();
    actual.sort_unstable();
    assert_eq!(actual, expected);
    Ok(())
}

// Remove all should remove all in this path.
async fn test_remove_all(op: Operator) -> Result<()> {
    let expected = vec![
        "x/", "x/y", "x/x/", "x/x/y", "x/x/x/", "x/x/x/y", "x/x/x/x/",
    ];
    for path in expected.iter() {
        op.object(path).create().await?;
    }

    let _ = op.batch().remove_all("x/").await?;

    for path in expected.iter() {
        if path.ends_with('/') {
            continue;
        }
        assert!(
            !op.object(path).is_exist().await?,
            "{path} should be removed"
        )
    }
    Ok(())
}
