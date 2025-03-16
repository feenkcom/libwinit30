use shared_library_builder::{GitLocation, LibraryLocation, RustLibrary};

pub fn libwinit(version: Option<impl Into<String>>) -> RustLibrary {
    RustLibrary::new(
        "Winit30",
        LibraryLocation::Git(GitLocation::github("feenkcom", "libwinit30").tag_or_latest(version)),
    )
    .package("libwinit")
}

pub fn latest_libwinit() -> RustLibrary {
    let version: Option<String> = None;
    libwinit(version)
}
