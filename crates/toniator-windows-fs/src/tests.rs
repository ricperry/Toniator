use super::*;
use std::fs;
use std::io::{Read, Seek, Write};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use windows_sys::Win32::Security::Authorization::{GetSecurityInfo, SE_FILE_OBJECT};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACL_SIZE_INFORMATION, AclSizeInformation, DACL_SECURITY_INFORMATION,
    GetAce, GetAclInformation, GetSecurityDescriptorControl, OWNER_SECURITY_INFORMATION,
    SE_DACL_PROTECTED,
};
use windows_sys::Win32::System::SystemServices::ACCESS_ALLOWED_ACE_TYPE;

/// Creates a unique retained artifact root without deleting prior native evidence.
/// # Panics
/// Panics if the native random source or scratch directory creation fails.
fn root() -> PathBuf {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/windows-port/fs-support/artifacts");
    fs::create_dir_all(&base).unwrap();
    let path = base.join(format!("native-{}", random_suffix().unwrap()));
    fs::create_dir(&path).unwrap();
    println!("retained native test artifacts: {}", path.display());
    fs::canonicalize(path).unwrap()
}

/// Writes exact test bytes through the retained regular-file handle.
/// # Panics
/// Panics on duplication/write/file-sync failure.
fn write(entry: &Entry, bytes: &[u8]) {
    let mut file = entry.try_clone_file().unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

/// Reads exact test bytes through a retained regular-file duplicate rather than a path.
/// # Panics
/// Panics on duplication/rewind/read failure.
fn read(entry: &Entry) -> Vec<u8> {
    let mut file = entry.try_clone_file().unwrap();
    file.rewind().unwrap();
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();
    bytes
}

/// Verifies a protected single-ACE DACL whose full-control trustee is the current user.
/// # Panics
/// Panics if native security queries fail or ownership/privacy assertions fail.
fn assert_private(file: &File) {
    let mut dacl = null_mut();
    let mut descriptor = null_mut();
    assert_eq!(
        unsafe {
            GetSecurityInfo(
                file.as_raw_handle(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut dacl,
                null_mut(),
                &mut descriptor,
            )
        },
        0
    );
    let _allocation = LocalAllocation(descriptor);
    let mut control = 0;
    let mut revision = 0;
    assert_ne!(
        unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) },
        0
    );
    assert_ne!(control & SE_DACL_PROTECTED, 0);
    assert!(!dacl.is_null());
    let mut information = ACL_SIZE_INFORMATION::default();
    assert_ne!(
        unsafe {
            GetAclInformation(
                dacl,
                (&mut information as *mut ACL_SIZE_INFORMATION).cast(),
                size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            )
        },
        0
    );
    assert_eq!(information.AceCount, 1);
    let mut ace = null_mut();
    assert_ne!(unsafe { GetAce(dacl, 0, &mut ace) }, 0);
    let ace = ace.cast::<ACCESS_ALLOWED_ACE>();
    assert_eq!(
        unsafe { (*ace).Header.AceType },
        ACCESS_ALLOWED_ACE_TYPE as u8
    );
    assert_eq!(unsafe { (*ace).Header.AceFlags }, 0);
    assert_eq!(unsafe { (*ace).Mask }, FILE_ALL_ACCESS);
    let sid = unsafe { addr_of!((*ace).SidStart).cast_mut().cast() };
    let mut sid_text = null_mut();
    assert_ne!(unsafe { ConvertSidToStringSidW(sid, &mut sid_text) }, 0);
    let _sid_allocation = LocalAllocation(sid_text.cast());
    let mut length = 0;
    while unsafe { *sid_text.add(length) } != 0 {
        length += 1;
    }
    let actual = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(sid_text, length) });
    assert_eq!(actual, current_user_sid().unwrap());
}

/// Proves child creation/publication remains bound to the retained directory after path replacement.
/// # Panics
/// Panics if the native operations or anchored ownership assertions fail.
#[test]
fn retained_directory_survives_rename_and_replacement() {
    let path = root();
    let original = path.join("selected");
    fs::create_dir(&original).unwrap();
    let directory = Directory::open(&original).unwrap();
    let moved = path.join("moved");
    fs::rename(&original, &moved).unwrap();
    fs::create_dir(&original).unwrap();
    assert!(directory.ensure_path(&original).is_err());
    let mut entry = directory.create_file(OsStr::new("staging")).unwrap();
    write(&entry, b"anchored");
    entry
        .publish(&directory, OsStr::new("completed"), PublishMode::NoReplace)
        .unwrap();
    assert_eq!(fs::read(moved.join("completed")).unwrap(), b"anchored");
    assert!(!original.join("completed").exists());
    assert!(directory.available_bytes().unwrap() > 0);
    directory.sync().unwrap();
}

/// Proves private file/directory ACLs exist at exclusive creation and collisions preserve contents.
/// # Panics
/// Panics if ACL, exclusive-creation, or retained-file assertions fail.
#[test]
fn exclusive_creation_has_protected_current_user_acl() {
    let path = root();
    let parent = Directory::open(&path).unwrap();
    let directory = parent.create_directory(OsStr::new("private")).unwrap();
    let file = directory.create_file(OsStr::new("private.bin")).unwrap();
    assert_private(&directory.inner.file);
    assert_private(file.file());
    write(&file, b"preserved");
    assert_eq!(
        directory
            .create_file(OsStr::new("private.bin"))
            .unwrap_err()
            .kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(
        parent
            .create_directory(OsStr::new("private"))
            .unwrap_err()
            .kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(read(&file), b"preserved");
}

/// Proves file publication refuses collisions and replacement occurs only when requested.
/// # Panics
/// Panics if native collision/replacement or output-content assertions fail.
#[test]
fn publication_collision_and_explicit_replacement() {
    let path = root();
    let parent = Directory::open(&path).unwrap();
    fs::write(path.join("final.bin"), b"user output").unwrap();
    let mut entry = parent.create_file(OsStr::new("staging.bin")).unwrap();
    write(&entry, b"new output");
    assert_eq!(
        entry
            .publish(&parent, OsStr::new("final.bin"), PublishMode::NoReplace)
            .unwrap_err()
            .kind(),
        io::ErrorKind::AlreadyExists
    );
    assert!(!entry.is_published());
    assert_eq!(fs::read(path.join("final.bin")).unwrap(), b"user output");
    entry
        .publish(&parent, OsStr::new("final.bin"), PublishMode::Replace)
        .unwrap();
    assert!(entry.is_published());
    assert_eq!(fs::read(path.join("final.bin")).unwrap(), b"new output");
    assert!(entry.discard().is_err());
    assert!(path.join("final.bin").exists());
}

/// Proves rename success revokes cleanup authority even when parent synchronization reports failure.
/// # Panics
/// Panics if the production publication transition or final-output preservation fails.
#[test]
fn publication_survives_parent_sync_failure() {
    let path = root();
    let parent = Directory::open(&path).unwrap();
    let mut entry = parent.create_file(OsStr::new("staging.bin")).unwrap();
    write(&entry, b"already published");
    let result = entry.publish_with_sync(
        &parent,
        OsStr::new("final.bin"),
        PublishMode::NoReplace,
        |_| Err(io::Error::other("injected parent synchronization failure")),
    );
    assert!(result.is_err());
    assert!(entry.is_published());
    assert!(entry.discard().is_err());
    assert_eq!(
        fs::read(path.join("final.bin")).unwrap(),
        b"already published"
    );
}

/// Proves replaced staging cannot redirect publication or explicit cleanup into a foreign file.
/// # Panics
/// Panics if ownership checks fail to retain either the original or foreign replacement.
#[test]
fn staging_replacement_revokes_publication_and_cleanup() {
    let path = root();
    let parent = Directory::open(&path).unwrap();
    let mut entry = parent.create_file(OsStr::new("staging.bin")).unwrap();
    write(&entry, b"original retained");
    fs::rename(path.join("staging.bin"), path.join("moved.bin")).unwrap();
    fs::write(path.join("staging.bin"), b"foreign replacement").unwrap();
    assert!(
        entry
            .publish(&parent, OsStr::new("final.bin"), PublishMode::NoReplace)
            .is_err()
    );
    assert!(entry.discard().is_err());
    assert_eq!(
        fs::read(path.join("staging.bin")).unwrap(),
        b"foreign replacement"
    );
    assert_eq!(
        fs::read(path.join("moved.bin")).unwrap(),
        b"original retained"
    );
    assert!(!path.join("final.bin").exists());
}

/// Proves explicit deletion removes only owned unpublished entries and refuses nonempty directories.
/// # Panics
/// Panics if handle-bound deletion or retained unknown-entry assertions fail.
#[test]
fn explicit_deletion_and_nonempty_directory_refusal() {
    let path = root();
    let parent = Directory::open(&path).unwrap();
    let directory = parent.create_directory(OsStr::new("work")).unwrap();
    let entry = directory.create_file(OsStr::new("owned.bin")).unwrap();
    write(&entry, b"owned");
    fs::write(path.join("work/unknown.bin"), b"unknown").unwrap();
    assert!(directory.clone().remove_empty().is_err());
    entry.discard().unwrap();
    assert!(!path.join("work/owned.bin").exists());
    assert_eq!(fs::read(path.join("work/unknown.bin")).unwrap(), b"unknown");
    directory
        .open_file(OsStr::new("unknown.bin"))
        .unwrap()
        .discard()
        .unwrap();
    directory.remove_empty().unwrap();
    assert!(!path.join("work").exists());
}

/// Proves bounds stop enumeration without deleting unknown entries or retained partials on drop.
/// # Panics
/// Panics if caller bounds, native enumeration, or retention assertions fail.
#[test]
fn enumeration_bounds_and_drop_preserve_entries() {
    let path = root();
    let parent = Directory::open(&path).unwrap();
    let partial = parent.create_file(OsStr::new("caller.partial")).unwrap();
    write(&partial, b"partial");
    drop(partial);
    fs::write(path.join("unexpected.bin"), b"unknown").unwrap();
    assert!(parent.names(1, 4096).is_err());
    assert!(parent.names(10, 2).is_err());
    let mut names = parent.names(2, 4096).unwrap();
    names.sort();
    assert_eq!(
        names,
        [
            OsString::from("caller.partial"),
            OsString::from("unexpected.bin")
        ]
    );
    drop(parent);
    assert_eq!(fs::read(path.join("caller.partial")).unwrap(), b"partial");
    assert_eq!(fs::read(path.join("unexpected.bin")).unwrap(), b"unknown");
}

/// Proves continuation across native query batches returns every unique name within exact caller bounds.
/// # Panics
/// Panics if native creation/enumeration or exact/one-below bounds assertions fail.
#[test]
fn enumeration_continues_across_kernel_batches() {
    let path = root();
    let parent = Directory::open(&path).unwrap();
    let mut expected = Vec::new();
    let mut name_bytes = 0;
    for index in 0..140 {
        let name = OsString::from(format!("{index:03}-{}", "a".repeat(240)));
        name_bytes += name.encode_wide().count() * 2;
        drop(parent.create_file(&name).unwrap());
        expected.push(name);
    }
    assert!(name_bytes > 64 * 1024);
    let mut actual = parent.names(expected.len(), name_bytes).unwrap();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    assert!(parent.names(expected.len() - 1, name_bytes).is_err());
    assert!(parent.names(expected.len(), name_bytes - 1).is_err());
}

/// Proves regular-file and directory opens reject the opposite kind without changing either object.
/// # Panics
/// Panics if native creation or wrong-kind/retention assertions fail.
#[test]
fn wrong_kind_opens_preserve_objects() {
    let path = root();
    let parent = Directory::open(&path).unwrap();
    drop(parent.create_directory(OsStr::new("directory")).unwrap());
    let entry = parent.create_file(OsStr::new("regular.bin")).unwrap();
    write(&entry, b"retained");
    assert!(parent.open_file(OsStr::new("directory")).is_err());
    assert!(parent.open_directory(OsStr::new("regular.bin")).is_err());
    assert!(Directory::open(&path.join("regular.bin")).is_err());
    assert_eq!(read(&entry), b"retained");
}

/// Proves publication/deletion affect only the selected link, including a moved original with a same-file alias.
/// # Panics
/// Panics if native deletion removes a different hard link or changes the retained contents.
#[test]
fn publication_and_deletion_preserve_other_hard_links() {
    let path = root();
    let parent = Directory::open(&path).unwrap();
    let entry = parent.create_file(OsStr::new("staging.bin")).unwrap();
    write(&entry, b"linked original");
    fs::hard_link(path.join("staging.bin"), path.join("alias.bin")).unwrap();
    entry.discard().unwrap();
    assert!(!path.join("staging.bin").exists());
    assert_eq!(
        fs::read(path.join("alias.bin")).unwrap(),
        b"linked original"
    );

    let entry = parent.create_file(OsStr::new("second-stage.bin")).unwrap();
    write(&entry, b"moved original");
    fs::rename(path.join("second-stage.bin"), path.join("moved.bin")).unwrap();
    fs::hard_link(path.join("moved.bin"), path.join("second-stage.bin")).unwrap();
    entry.discard().unwrap();
    assert_eq!(fs::read(path.join("moved.bin")).unwrap(), b"moved original");
    assert!(!path.join("second-stage.bin").exists());
    let mut entry = parent.create_file(OsStr::new("publish-stage.bin")).unwrap();
    write(&entry, b"published alias");
    fs::rename(path.join("publish-stage.bin"), path.join("held.bin")).unwrap();
    fs::hard_link(path.join("held.bin"), path.join("publish-stage.bin")).unwrap();
    entry
        .publish(&parent, OsStr::new("published.bin"), PublishMode::NoReplace)
        .unwrap();
    assert_eq!(fs::read(path.join("held.bin")).unwrap(), b"published alias");
    assert_eq!(
        fs::read(path.join("published.bin")).unwrap(),
        b"published alias"
    );
    assert!(!path.join("publish-stage.bin").exists());
    assert!(entry.discard().is_err());
}

/// Proves root and component validation prevents NUL truncation, traversal, and alternate-stream opens.
/// # Panics
/// Panics if an invalid component is accepted or the directory changes.
#[test]
fn relative_component_validation_is_bounded() {
    let path = root();
    let mut truncated_path: Vec<u16> = path.as_os_str().encode_wide().collect();
    truncated_path.push(0);
    truncated_path.extend("ignored".encode_utf16());
    assert_eq!(
        Directory::open(&PathBuf::from(OsString::from_wide(&truncated_path)))
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
    let parent = Directory::open(&path).unwrap();
    for name in [
        "",
        ".",
        "..",
        "../outside",
        "nested/file",
        "nested\\file",
        "file:stream",
        "nul\0name",
    ] {
        assert_eq!(
            parent.create_file(OsStr::new(name)).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }
    assert!(parent.names(0, 0).unwrap().is_empty());
}

/// Proves relevant directory junction leaves cannot redirect root or handle-relative child opens.
/// # Panics
/// Panics if hidden junction creation fails or a no-follow boundary accepts the junction.
#[test]
fn junction_leaves_are_rejected_without_following() {
    let path = root();
    let target = path.join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("sentinel.bin"), b"outside target").unwrap();
    let junction = path.join("junction");
    let output = Command::new(std::env::var_os("ComSpec").unwrap())
        .args([
            OsStr::new("/d"),
            OsStr::new("/c"),
            OsStr::new("mklink"),
            OsStr::new("/J"),
            junction.as_os_str(),
            target.as_os_str(),
        ])
        .creation_flags(0x08000000)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(Directory::open(&junction).is_err());
    assert!(available_space(&junction).is_err());
    assert!(open_regular_file(&junction).is_err());
    let parent = Directory::open(&path).unwrap();
    assert!(parent.open_directory(OsStr::new("junction")).is_err());
    assert!(parent.open_file(OsStr::new("junction")).is_err());
    assert_eq!(
        fs::read(target.join("sentinel.bin")).unwrap(),
        b"outside target"
    );
}

/// Queries capacity and reads regular files without creation or deletion rights on the selected leaf.
/// # Panics
/// Panics if ACL setup fails, a read-only capacity query requires write access, or wrong-kind reads succeed.
#[test]
fn capacity_and_regular_reads_need_no_creation_rights() {
    let path = root();
    let file = path.join("read-only.bin");
    fs::write(&file, b"read-only data").unwrap();
    let sid = current_user_sid().unwrap();
    for leaf in [&file, &path] {
        let output = Command::new("icacls.exe")
            .arg(leaf)
            .args(["/inheritance:r", "/grant:r"])
            .arg(format!("*{sid}:(R)"))
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(Directory::open(&path).is_err());
    assert!(available_space(&path).unwrap() > 0);
    assert!(available_space(&file).unwrap() > 0);
    let mut opened = open_regular_file(&file).unwrap();
    let mut bytes = Vec::new();
    opened.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"read-only data");
    assert!(open_regular_file(&path).is_err());
}

/// Exercises rename independently of synchronization and irrevocably revokes staging deletion.
/// # Panics
/// Panics if rename requires an implicit sync or cleanup authority survives publication.
#[test]
fn rename_only_publication_latches_ownership() {
    let path = root();
    let parent = Directory::open(&path).unwrap();
    assert!(!parent.entry_exists(OsStr::new("partial.bin")).unwrap());
    let mut entry = parent.create_file(OsStr::new("partial.bin")).unwrap();
    assert!(parent.entry_exists(OsStr::new("partial.bin")).unwrap());
    write(&entry, b"published independently");
    entry
        .rename(&parent, OsStr::new("final.bin"), PublishMode::NoReplace)
        .unwrap();
    assert!(entry.is_published());
    assert!(entry.discard().is_err());
    assert!(!path.join("partial.bin").exists());
    assert_eq!(
        fs::read(path.join("final.bin")).unwrap(),
        b"published independently"
    );
    parent.sync().unwrap();
}

/// Keeps retained old readers valid while atomic replacement makes new opens observe the new bytes.
/// # Panics
/// Panics if replacement fails with an open reader, mutates its old bytes, or exposes mixed content.
#[test]
fn replacement_preserves_retained_readers() {
    let path = root();
    let parent = Directory::open(&path).unwrap();
    let mut first = parent.create_file(OsStr::new("first.tmp")).unwrap();
    write(&first, b"old bytes");
    first
        .rename(&parent, OsStr::new("visible.bin"), PublishMode::NoReplace)
        .unwrap();
    let reader = parent.open_file(OsStr::new("visible.bin")).unwrap();
    let mut second = parent.create_file(OsStr::new("second.tmp")).unwrap();
    write(&second, b"new bytes");
    second
        .rename(&parent, OsStr::new("visible.bin"), PublishMode::Replace)
        .unwrap();
    assert_eq!(read(&reader), b"old bytes");
    assert_eq!(fs::read(path.join("visible.bin")).unwrap(), b"new bytes");
    parent.sync().unwrap();
}

/// Opens independent cursors from an NT-created retained object even after its visible link is replaced.
/// # Panics
/// Panics if native reopen fails, cursors interfere, or a replaced pathname redirects retained reads.
#[test]
fn independent_readers_survive_path_replacement() {
    let path = root();
    let parent = Directory::open(&path).unwrap();
    let entry = parent.create_file(OsStr::new("input.media")).unwrap();
    write(&entry, b"abcdef");
    let mut first = entry.independent_reader().unwrap();
    let mut second = entry.independent_reader().unwrap();
    let mut byte = [0];
    first.read_exact(&mut byte).unwrap();
    assert_eq!(&byte, b"a");
    first.read_exact(&mut byte).unwrap();
    assert_eq!(&byte, b"b");
    second.read_exact(&mut byte).unwrap();
    assert_eq!(&byte, b"a");
    fs::rename(path.join("input.media"), path.join("moved.media")).unwrap();
    fs::write(path.join("input.media"), b"replacement").unwrap();
    let mut third = entry.independent_reader().unwrap();
    third.read_exact(&mut byte).unwrap();
    assert_eq!(&byte, b"a");
    assert!(entry.discard().is_err());
    assert_eq!(fs::read(path.join("input.media")).unwrap(), b"replacement");
}
