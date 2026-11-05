use launcher_core::{FileCheck, Hash, HashAlgo, Location, Origin, Recipe, Root, Step};
use url::Url;

fn location(root: Root, rel: &str) -> Location {
    Location {
        root,
        rel: rel.to_owned(),
    }
}

#[test]
fn outline_lists_download_steps_and_checks_in_order() {
    let recipe = Recipe {
        version: "1.0".to_owned(),
        origin: Origin::Direct(Url::parse("https://x.example/a.zip").unwrap()),
        hash: None,
        steps: vec![
            Step::Extract,
            Step::Copy {
                from: location(Root::Tmp, "a.auf2"),
                to: location(Root::Data, "Plugin"),
            },
        ],
        checks: vec![FileCheck {
            location: location(Root::Data, "Plugin/a.auf2"),
            hash: Hash {
                algo: HashAlgo::Xxh3_128,
                hex: "00ff".to_owned(),
            },
        }],
    };
    assert_eq!(
        recipe.outline(),
        [
            "download https://x.example/a.zip",
            "extract zip -> tmp",
            "copy tmp/a.auf2 -> data/Plugin",
            "check Xxh3_128 data/Plugin/a.auf2 = 00ff",
        ]
    );
}

#[test]
fn outline_names_the_root_without_a_path() {
    let recipe = Recipe {
        version: "1".to_owned(),
        origin: Origin::GithubRelease {
            owner: "o".to_owned(),
            repo: "r".to_owned(),
            pattern: None,
        },
        hash: None,
        steps: vec![Step::RunAuoSetup {
            path: location(Root::App, ""),
        }],
        checks: Vec::new(),
    };
    assert_eq!(
        recipe.outline(),
        ["download github:o/r", "run auo_setup2 app"]
    );
}
