//! [`crate::CloudStorageProvider`] implementations: the user-visible Local folder
//! provider, Google Drive and, for tests, the in-memory fake.

#[cfg(any(test, feature = "test-support"))]
pub mod fake_cloud_provider;
#[cfg(any(test, feature = "test-support"))]
mod fake_cloud_provider_ops;
pub mod google_drive_account;
mod google_drive_downloads;
mod google_drive_errors;
mod google_drive_path_ids;
pub mod google_drive_provider;
mod google_drive_requests;
mod google_drive_resumable_upload;
mod google_drive_uploads;
pub mod local_folder_provider;

#[cfg(test)]
mod google_drive_error_tests;
#[cfg(test)]
mod google_drive_executor_tests;
#[cfg(test)]
mod google_drive_fake_server;
#[cfg(test)]
mod google_drive_fake_server_store;
#[cfg(test)]
mod google_drive_live_test;
#[cfg(test)]
mod google_drive_provider_tests;
#[cfg(test)]
mod google_drive_test_support;
#[cfg(test)]
mod google_drive_transfer_tests;
#[cfg(test)]
mod google_drive_upload_tests;
