use super::*;

/// (season, part, final, year) of a title.
fn marks(title: &str) -> (Option<u32>, Option<u32>, bool, Option<i32>) {
    let info = parse_title(title);
    (info.season, info.part, info.final_season, info.year)
}

#[test]
fn seasons_in_english_titles() {
    let cases: &[(&str, Option<u32>, &str)] = &[
        ("Overlord", None, "overlord"),
        ("Overlord II", Some(2), "overlord"),
        ("Overlord III", Some(3), "overlord"),
        ("Overlord IV", Some(4), "overlord"),
        ("overlord ii", Some(2), "overlord"),
        ("Attack on Titan Season 2", Some(2), "attack on titan"),
        ("Attack on Titan: Season 2", Some(2), "attack on titan"),
        ("Attack on Titan - Season 02", Some(2), "attack on titan"),
        ("Attack on Titan (Season 2)", Some(2), "attack on titan"),
        ("Haikyu!! 2nd Season", Some(2), "haikyu"),
        ("Haikyuu!! Second Season", Some(2), "haikyuu"),
        ("Kuroko no Basket 3rd Season", Some(3), "kuroko no basket"),
        ("Fate/Zero 2nd Season", Some(2), "fate zero"),
        (
            "Monogatari Series: Second Season",
            Some(2),
            "monogatari series",
        ),
        ("Sherlock Series 2", Some(2), "sherlock"),
        ("Initial D Fourth Stage", Some(4), "initial d"),
        ("Spy x Family Season Two", Some(2), "spy x family"),
        ("Jujutsu Kaisen S2", Some(2), "jujutsu kaisen"),
        ("Jujutsu Kaisen S02", Some(2), "jujutsu kaisen"),
        ("Sword Art Online II", Some(2), "sword art online"),
        ("Psycho-Pass 3", Some(3), "psycho pass"),
        (
            "Psycho-Pass 3: First Inspector",
            Some(3),
            "psycho pass first inspector",
        ),
        ("Bungo Stray Dogs 2", Some(2), "bungo stray dogs"),
        ("Date A Live V", Some(5), "date a live"),
        ("Mob Psycho 100", None, "mob psycho 100"),
        ("Mob Psycho 100 III", Some(3), "mob psycho 100"),
        ("Steins;Gate 0", None, "steins gate 0"),
        ("86 Eighty-Six", None, "86 eighty six"),
        ("Mobile Suit Gundam X", None, "mobile suit gundam x"),
        (
            "Mobile Suit Gundam 00 Second Season",
            Some(2),
            "mobile suit gundam 00",
        ),
        (
            "Is It Wrong to Try to Pick Up Girls in a Dungeon? IV Part 2",
            Some(4),
            "is it wrong to try to pick up girls in a dungeon",
        ),
        (
            "Re:ZERO -Starting Life in Another World- Season 3",
            Some(3),
            "re zero starting life in another world",
        ),
        ("The Seven Deadly Sins", None, "seven deadly sins"),
        ("Detective Conan Movie 2", None, "detective conan movie 2"),
        (
            "Star Wars: Episode IV - A New Hope",
            None,
            "star wars episode iv a new hope",
        ),
    ];
    for (title, season, base) in cases {
        let info = parse_title(title);
        assert_eq!(info.season, *season, "season of {title}");
        assert_eq!(info.base, *base, "base of {title}");
    }
}

#[test]
fn seasons_before_a_subtitle_and_in_romaji() {
    let info = parse_title("Mushoku Tensei II: Isekai Ittara Honki Dasu");
    assert_eq!(info.season, Some(2));
    assert_eq!(info.base, "mushoku tensei isekai ittara honki dasu");
    assert_eq!(info.name, "Mushoku Tensei Isekai Ittara Honki Dasu");
    assert_eq!(
        marks("Mushoku Tensei II: Isekai Ittara Honki Dasu Part 2"),
        (Some(2), Some(2), false, None)
    );
    assert_eq!(
        marks("Mushoku Tensei: Isekai Ittara Honki Dasu 2nd Season Part 2"),
        (Some(2), Some(2), false, None)
    );
    assert_eq!(
        marks("Mushoku Tensei: Jobless Reincarnation Season 2 Cour 2"),
        (Some(2), Some(2), false, None)
    );
    assert_eq!(
        marks("Mushoku Tensei: Jobless Reincarnation Part 2"),
        (None, Some(2), false, None)
    );
    assert_eq!(marks("Spy x Family Part 2"), (None, Some(2), false, None));
    assert_eq!(
        marks("Attack on Titan Season 3 Part 2"),
        (Some(3), Some(2), false, None)
    );
    assert_eq!(
        marks("Shingeki no Kyojin Season 3 (2019)"),
        (Some(3), None, false, Some(2019))
    );
    assert_eq!(marks("SnK 2"), (Some(2), None, false, None));
    assert_eq!(
        marks("Tensei shitara Slime Datta Ken 2nd Season Part 2"),
        (Some(2), Some(2), false, None)
    );
}

#[test]
fn final_seasons_years_and_decorations() {
    let final_season = parse_title("Attack on Titan: The Final Season");
    assert!(final_season.final_season);
    assert_eq!(final_season.base, "attack on titan");
    assert_eq!(
        marks("Attack on Titan: The Final Season - Final Chapters Part 2"),
        (None, Some(2), true, None)
    );
    assert_eq!(
        base_title("Attack on Titan: The Final Season - Final Chapters Part 2"),
        "attack on titan final chapters"
    );
    assert_eq!(marks("Initial D Final Stage"), (None, None, true, None));
    assert_eq!(
        marks("Hunter x Hunter (2011)"),
        (None, None, false, Some(2011))
    );
    assert_eq!(
        marks("Hunter x Hunter 2011"),
        (None, None, false, Some(2011))
    );
    assert_eq!(base_title("Hunter × Hunter (2011)"), "hunter x hunter");
    assert_eq!(base_title("Jujutsu Kaisen (Dub)"), "jujutsu kaisen");
    assert_eq!(base_title("Jujutsu Kaisen (English Dub)"), "jujutsu kaisen");
    assert_eq!(base_title("Black Clover (TV)"), "black clover");
    assert_eq!(base_title("Naruto Shippuden Dub"), "naruto shippuden");
    assert_eq!(base_title("Spice & Wolf"), "spice and wolf");
    // A bracketed name that isn't a version stays.
    assert_eq!(
        base_title("Fate/stay night [Unlimited Blade Works]"),
        "fate stay night unlimited blade works"
    );
    assert_eq!(marks("2001: A Space Odyssey"), (None, None, false, None));
}

#[test]
fn seasons_in_japanese_chinese_korean_and_full_width_titles() {
    assert_eq!(season_of_title("進撃の巨人 第2期"), Some(2));
    assert_eq!(season_of_title("進撃の巨人 Season 2"), Some(2));
    assert_eq!(
        season_of_title("無職転生 ～異世界行ったら本気だす～ 第3期"),
        Some(3)
    );
    assert_eq!(season_of_title("ダンジョン飯 2期"), Some(2));
    assert_eq!(season_of_title("鬼灭之刃 第二季"), Some(2));
    assert_eq!(season_of_title("某科学的超电磁炮 第十一季"), Some(11));
    assert_eq!(season_of_title("呪術廻戦 2nd シーズン"), Some(2));
    assert_eq!(season_of_title("나 혼자만 레벨업 시즌 2"), Some(2));
    assert_eq!(base_title("나 혼자만 레벨업 시즌 2"), "나 혼자만 레벨업");
    assert_eq!(season_of_title("Ⅱ期 test"), None);
    assert_eq!(season_of_title("Ｏｖｅｒｌｏｒｄ Ⅱ"), Some(2));
    assert_eq!(season_of_title("ゆるキャン△ 第２期"), Some(2));
    assert_eq!(marks("SPY×FAMILY 第2クール"), (None, Some(2), false, None));
    let info = parse_title("進撃の巨人 第2期");
    assert_eq!(info.base, "進撃の巨人");
    assert_eq!(info.key(), parse_title("進撃の巨人 Season 2").key());
}

#[test]
fn same_title_respects_the_season() {
    let same = [
        (
            "Mushoku Tensei II: Isekai Ittara Honki Dasu",
            "Mushoku Tensei: Isekai Ittara Honki Dasu 2nd Season",
        ),
        ("Overlord II", "Overlord Season 2"),
        ("Overlord II", "OVERLORD 2nd Season"),
        ("Attack on Titan Season 2", "Attack on Titan II"),
        ("Attack on Titan Season 2", "Attack on Titan S2"),
        ("Frieren", "Frieren (Dub)"),
        ("Frieren", "Frieren Season 1"),
        ("Overlord I", "Overlord"),
        ("Shingeki no Kyojin", "Shingeki no Kyoujin"),
        ("Haikyuu!!", "Haikyu!!"),
        ("Re:Zero", "ReZero"),
        ("Dr. Stone", "Dr Stone"),
        ("Spy×Family Season 2", "Spy x Family Season 2"),
        (
            "Attack on Titan Season 3 Part 1",
            "Attack on Titan Season 3",
        ),
        ("Fruits Basket 1st Season", "Fruits Basket (2019)"),
        ("Hunter x Hunter (2011)", "Hunter x Hunter"),
        ("The Seven Deadly Sins", "Seven Deadly Sins"),
    ];
    for (a, b) in same {
        assert!(same_title(a, b), "{a} and {b} are the same season");
        assert!(same_title(b, a), "{b} and {a} are the same season");
    }
    let different = [
        ("Overlord", "Overlord II"),
        (
            "Mushoku Tensei: Isekai Ittara Honki Dasu",
            "Mushoku Tensei II: Isekai Ittara Honki Dasu",
        ),
        ("Attack on Titan Season 2", "Attack on Titan Season 3"),
        (
            "Attack on Titan Season 3",
            "Attack on Titan Season 3 Part 2",
        ),
        (
            "Attack on Titan Season 4",
            "Attack on Titan: The Final Season",
        ),
        ("Attack on Titan", "Attack on Titan OVA"),
        ("Attack on Titan", "Attack on Titan: Junior High"),
        ("Fruits Basket (2001)", "Fruits Basket (2019)"),
        ("Fruits Basket (2019)", "Fruits Basket 2nd Season"),
        ("Mob Psycho 100", "Mob Psycho 100 II"),
        ("Naruto", "Naruto Shippuden"),
        ("Spy x Family", "Spy x Family Part 2"),
        ("", ""),
    ];
    for (a, b) in different {
        assert!(!same_title(a, b), "{a} and {b} differ");
        assert!(!same_title(b, a), "{b} and {a} differ");
    }
}

/// (season, episode) of a name.
fn numbers(name: &str) -> (Option<u32>, Option<f64>) {
    let info = parse_episode(name);
    (info.season, info.episode)
}

#[test]
fn episodes_in_release_and_file_names() {
    let cases: &[(&str, Option<u32>, Option<f64>)] = &[
        (
            "Show.Name.S01E02.1080p.WEB-DL.x264-GROUP.mkv",
            Some(1),
            Some(2.0),
        ),
        ("Show Name - S02E05 - The Return", Some(2), Some(5.0)),
        ("show s2e5", Some(2), Some(5.0)),
        ("Show S01 E02", Some(1), Some(2.0)),
        ("Show 1x03", Some(1), Some(3.0)),
        ("Movie 1920x1080", None, None),
        (
            "[SubsPlease] Sousou no Frieren - 12 (1080p) [ABCD1234].mkv",
            None,
            Some(12.0),
        ),
        (
            "[Erai-raws] Show - 03v2 [1080p][Multiple Subtitle].mkv",
            None,
            Some(3.0),
        ),
        ("Mushoku Tensei II - 03", Some(2), Some(3.0)),
        ("[Group] Show 2nd Season - 03 [720p]", Some(2), Some(3.0)),
        ("Show S2 - 03", Some(2), Some(3.0)),
        ("Show Season 2 - 03", Some(2), Some(3.0)),
        ("Show (Season 2) - 03", Some(2), Some(3.0)),
        ("Show Season 2 Episode 5", Some(2), Some(5.0)),
        ("Anime - Episode 12", None, Some(12.0)),
        ("Anime Ep. 12.5", None, Some(12.5)),
        ("Anime EP07", None, Some(7.0)),
        ("One Piece - 1071 [1080p]", None, Some(1071.0)),
        ("進撃の巨人 第3話", None, Some(3.0)),
        ("Show #4", None, Some(4.0)),
        ("Show 03", None, Some(3.0)),
        ("Show 03 1080p", None, Some(3.0)),
        ("05", None, Some(5.0)),
        ("Sintel", None, None),
        ("Big Buck Bunny 2008", None, None),
        ("Attack on Titan Season 1-4 Complete 1080p", None, None),
        ("Attack.on.Titan.S01-S04.1080p", None, None),
    ];
    for (name, season, episode) in cases {
        assert_eq!(numbers(name), (*season, *episode), "{name}");
    }
    let double = parse_episode("Show S01E01-E02 1080p");
    assert_eq!(
        (double.episode, double.last_episode),
        (Some(1.0), Some(2.0))
    );
    let batch = parse_episode("[Group] Show (01-12) [1080p]");
    assert_eq!((batch.episode, batch.last_episode), (Some(1.0), Some(12.0)));
    let dashed = parse_episode("[Group] Show - 01 ~ 12 [BD]");
    assert_eq!(
        (dashed.episode, dashed.last_episode),
        (Some(1.0), Some(12.0))
    );
    assert!(
        parse_episode("Show 03").weak,
        "a trailing number is only a hint"
    );
    assert!(!parse_episode("Show - 03").weak);
    assert_eq!(
        parse_episode("[SubsPlease] Sousou no Frieren - 12 (1080p)")
            .show
            .name,
        "Sousou no Frieren"
    );
    assert_eq!(
        parse_episode("S01E03 - The Return 1080p WEB").rest,
        "The Return"
    );
    assert_eq!(episode_of("Show II - 03"), Some((Some(2), 3.0)));
    assert_eq!(episode_of("Sintel"), None);
    // Text that isn't ASCII before a marker reads fine (slices fall on character boundaries).
    assert_eq!(episode_of("Café Ep 4"), Some((None, 4.0)));
    assert_eq!(episode_of("進撃の巨人 Episode 3"), Some((None, 3.0)));
    assert_eq!(episode_of("Ünïcödé – ep 5 – Fin"), Some((None, 5.0)));
    assert_eq!(episode_of("ÉÉÉ"), None);
    // "ep" inside a word, a number ending a name and a resolution are no episodes.
    assert_eq!(episode_of("Sleep 2"), None);
    assert_eq!(episode_of("Deep Sleep"), None);
    assert_eq!(episode_of("Movie 2"), None);
    assert_eq!(episode_of("Deep Space 1920x1080"), None);
}

#[test]
fn streams_naming_another_episode_are_told_apart() {
    // Attack on Titan S2E1 as Cinemeta numbers it, with its number across seasons.
    let wanted = [
        EpisodeNumber {
            season: Some(2),
            episode: 1,
        },
        EpisodeNumber {
            season: None,
            episode: 26,
        },
    ];
    let other = |text: &str| names_other_episode(text, &wanted);
    assert!(!other(
        "Torrentio\n1080p · Attack.on.Titan.S02E01.1080p.WEB.x264"
    ));
    assert!(!other("[SubsPlease] Shingeki no Kyojin S2 - 01 (1080p)"));
    assert!(
        !other("Shingeki no Kyojin - 26 [1080p]"),
        "the number across seasons"
    );
    assert!(!other(
        "Attack on Titan Season 2 [1080p]\nAttack on Titan - S02E01.mkv"
    ));
    assert!(
        !other("Attack on Titan Season 2 Complete"),
        "the right season's pack"
    );
    assert!(
        !other("Attack on Titan Seasons 1-4"),
        "several seasons can't be told"
    );
    assert!(!other("HD 1080p"), "nothing named");
    assert!(
        !other("Attack on Titan 2"),
        "a trailing number is only a hint"
    );
    assert!(other("Attack.on.Titan.S01E01.1080p"));
    assert!(
        other("Attack on Titan Season 1 [1080p]"),
        "another season's pack"
    );
    assert!(other("[SubsPlease] Shingeki no Kyojin - 05 (1080p)"));
    assert!(other("Attack on Titan S03E01"));
    assert!(
        !names_other_episode("Attack.on.Titan.S01E01", &[]),
        "nothing wanted"
    );
    // A batch holding the episode fits.
    assert!(!names_other_episode(
        "[Group] Show (01-12)",
        &[EpisodeNumber {
            season: None,
            episode: 5,
        }]
    ));
}

#[test]
fn episodes_go_by_season_then_number_with_specials_last() {
    let episode = |season: Option<f64>, number: Option<f64>| EpisodeOrder {
        season,
        number,
        date: None,
    };
    // Numbered per season, listed newest first: seasons don't interleave.
    let listed = [
        episode(Some(2.0), Some(2.0)),
        episode(Some(2.0), Some(1.0)),
        episode(Some(0.0), Some(1.0)),
        episode(Some(1.0), Some(2.0)),
        episode(Some(1.0), Some(1.0)),
        episode(None, Some(1.0)),
    ];
    assert_eq!(watching_order(&listed), [4, 3, 1, 0, 2, 5]);
    // Without seasons: by number.
    let plain = [
        episode(None, Some(3.0)),
        episode(None, Some(1.0)),
        episode(None, Some(2.0)),
    ];
    assert_eq!(watching_order(&plain), [1, 2, 0]);
    // Unnumbered: newest first unless the numbered ones rise.
    let newest_first = [
        episode(None, None),
        episode(None, Some(2.0)),
        episode(None, Some(1.0)),
    ];
    assert_eq!(watching_order(&newest_first), [2, 1, 0]);
    let oldest_first = [
        episode(None, Some(1.0)),
        episode(None, Some(2.0)),
        episode(None, None),
    ];
    assert_eq!(watching_order(&oldest_first), [0, 1, 2]);
    // Dated: by date.
    let dated = [
        EpisodeOrder {
            season: None,
            number: None,
            date: Some(30),
        },
        EpisodeOrder {
            season: None,
            number: None,
            date: Some(10),
        },
        EpisodeOrder {
            season: None,
            number: None,
            date: Some(20),
        },
    ];
    assert_eq!(watching_order(&dated), [1, 2, 0]);
    assert!(is_special(Some(0.0), true));
    assert!(is_special(None, true));
    assert!(!is_special(None, false));
    assert!(!is_special(Some(1.0), true));
}

#[test]
fn episode_labels_leave_out_names_that_repeat_the_number() {
    assert_eq!(
        episode_label(Some(1.0), Some(2.0), "Pilot"),
        "S1 E2 · Pilot"
    );
    assert_eq!(episode_label(Some(1.0), Some(2.0), "Episode 2"), "S1 E2");
    assert_eq!(episode_label(Some(1.0), Some(2.0), "S01E02"), "S1 E2");
    assert_eq!(episode_label(Some(1.0), Some(2.0), "2"), "S1 E2");
    assert_eq!(
        episode_label(Some(1.0), Some(2.0), "Episode 3"),
        "S1 E2 · Episode 3"
    );
    assert_eq!(
        episode_label(Some(1.0), Some(2.0), "Episode 2 - The Return"),
        "S1 E2 · Episode 2 - The Return"
    );
    assert_eq!(episode_label(None, Some(5.0), "Episode 5"), "Episode 5");
    assert_eq!(episode_label(None, Some(5.0), "第5話"), "E5");
    assert_eq!(
        episode_label(None, Some(5.0), "The Return"),
        "E5 · The Return"
    );
    assert_eq!(episode_label(None, Some(12.5), "Recap"), "E12.5 · Recap");
    assert_eq!(episode_label(Some(2.0), None, "Special"), "Special");
    assert_eq!(episode_label(None, None, "Film"), "Film");
}
