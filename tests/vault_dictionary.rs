//! A vault's own words, and the index that has to keep up with them.
//!
//! Two failures are guarded here, and only the second is obvious. The first is
//! that team vocabulary is unsearchable: a note titled `LHVendor` that `vendor`
//! does not find. The second is worse because it is silent — edit the word list
//! and every document already in the index was cut into terms by the old one,
//! so search answers with the wrong notes and reports nothing wrong at all.

use std::fs;
use std::process::Command;

fn samong(vault: &std::path::Path, config: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_samong"));
    command.current_dir(vault).env("SAMONG_CONFIG_DIR", config);
    command
}

fn search(vault: &std::path::Path, config: &std::path::Path, query: &str) -> String {
    let output = samong(vault, config)
        .args(["search", query, "--limit", "5"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn a_word_inside_a_word_is_searchable() {
    let root = tempfile::tempdir().unwrap();
    let vault = root.path().join("brain");
    let config = root.path().join("config");
    fs::create_dir_all(&vault).unwrap();
    fs::write(
        vault.join("vendor-jdbc.md"),
        "# LHVendor ต่อ Informix ไม่ได้\n\n\
         Error: Cannot load JDBC driver class 'com.informix.jdbc.IfxDriver'\n\
         asset LH2291\n",
    )
    .unwrap();

    // The measured failure: this note was not findable by the word inside its
    // own title.
    let hits = search(&vault, &config, "vendor");
    assert!(hits.contains("vendor-jdbc.md"), "{hits}");

    // The same for an acronym stuck to a word, and for an asset number.
    assert!(search(&vault, &config, "driver").contains("vendor-jdbc.md"));
    assert!(search(&vault, &config, "2291").contains("vendor-jdbc.md"));

    // And the whole word still works — splitting must add, never replace.
    assert!(search(&vault, &config, "LHVendor").contains("vendor-jdbc.md"));
}

#[test]
fn editing_the_word_list_rebuilds_the_index_instead_of_answering_from_a_stale_one() {
    let root = tempfile::tempdir().unwrap();
    let vault = root.path().join("brain");
    let config = root.path().join("config");
    fs::create_dir_all(&vault).unwrap();

    // A Thai compound the bundled dictionary does not know, written with no
    // spaces as Thai is: without help the segmenter cuts it into other words.
    let note = "# ระบบสมองกลอัจฉริยะ\n\nเอกสารของระบบนี้\n";
    fs::write(vault.join("system.md"), note).unwrap();
    fs::write(vault.join("samong.toml"), "[vault]\nname = \"brain\"\n").unwrap();

    // Index once with no vault words.
    search(&vault, &config, "ระบบ");

    // Now declare the compound. The index on disk was built by the old
    // dictionary; if nothing forces a rebuild, this search is answered from
    // terms that no longer match how the query is cut.
    fs::write(
        vault.join("samong.toml"),
        "[vault]\nname = \"brain\"\n\n[search]\nwords = [\"สมองกลอัจฉริยะ\"]\n",
    )
    .unwrap();

    let hits = search(&vault, &config, "สมองกลอัจฉริยะ");
    assert!(
        hits.contains("system.md"),
        "the declared word did not find the note, so the index was not rebuilt: {hits}"
    );

    // Taking the word away again must also rebuild, not leave the vault indexed
    // by a dictionary its config no longer describes.
    fs::write(vault.join("samong.toml"), "[vault]\nname = \"brain\"\n").unwrap();
    assert!(search(&vault, &config, "ระบบ").contains("system.md"));
}

#[test]
fn an_unknown_key_under_search_fails_loudly() {
    let root = tempfile::tempdir().unwrap();
    let vault = root.path().join("brain");
    let config = root.path().join("config");
    fs::create_dir_all(&vault).unwrap();
    fs::write(vault.join("a.md"), "# A\n").unwrap();
    fs::write(
        vault.join("samong.toml"),
        "[search]\nwordz = [\"typo in the key\"]\n",
    )
    .unwrap();

    let output = samong(&vault, &config)
        .args(["search", "A"])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "a misspelled config key must not be ignored"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("wordz"), "{stderr}");
}
