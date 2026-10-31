use std::sync::atomic::{AtomicI64, Ordering};

use raygun_db::collections::{self, TrackFields};
use tempfile::TempDir;

use super::*;

const LOCALHOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

struct Device {
    engine: Arc<Engine>,
    db: Arc<Db>,
    _server: Running,
    clock: Arc<AtomicI64>,
}

async fn device(temp: &TempDir, name: &str) -> Device {
    let db = Arc::new(
        Db::open(&temp.path().join(format!("{name}.db")))
            .await
            .unwrap(),
    );
    let clock = Arc::new(AtomicI64::new(1_000));
    let time = clock.clone();
    let engine = Arc::new(Engine::with_clock(
        db.clone(),
        Identity::generate().unwrap(),
        name.into(),
        SyncKind::ALL.to_vec(),
        Box::new(move || time.load(Ordering::SeqCst)),
    ));
    // Port 0: the OS picks a free one, so parallel tests never collide.
    let server = engine.serve_on(LOCALHOST, 0..=0).await.unwrap();
    Device {
        engine,
        db,
        _server: server,
        clock,
    }
}

fn song(key: &str) -> TrackFields {
    TrackFields {
        track_key: key.into(),
        title: key.rsplit('|').next().unwrap().into(),
        artist: Some(key.split('|').next().unwrap().into()),
        album: None,
        duration_ms: Some(1000),
        artwork_url: None,
        source: None,
    }
}

async fn pair(host: &Device, joiner: &Device) {
    let offer = host.engine.open_pairing(LOCALHOST).unwrap();
    joiner.engine.join(&offer).await.unwrap();
}

async fn titles(db: &Db) -> Vec<String> {
    let mut titles: Vec<String> = collections::favorites(db)
        .await
        .unwrap()
        .into_iter()
        .map(|favorite| favorite.track.title)
        .collect();
    titles.sort();
    titles
}

#[tokio::test]
async fn paired_devices_sync_both_ways() {
    let temp = TempDir::new().unwrap();
    let (phone, laptop) = (device(&temp, "phone").await, device(&temp, "laptop").await);
    pair(&phone, &laptop).await;

    let on_phone = sync::peers(&phone.db).await.unwrap();
    let on_laptop = sync::peers(&laptop.db).await.unwrap();
    assert_eq!(on_phone[0].name, "laptop");
    assert_eq!(on_laptop[0].name, "phone");
    assert_eq!(
        on_phone[0].public_key,
        to_hex(&laptop.engine.identity().public)
    );
    assert!(
        on_phone[0].address.is_some(),
        "the joiner's port came with the pairing"
    );

    collections::set_favorite(&phone.db, &song("nina|sinnerman"), true, 1)
        .await
        .unwrap();
    let playlist = collections::create_playlist(&laptop.db, "Road trip", 2)
        .await
        .unwrap();

    let summary = laptop
        .engine
        .sync_with(phone.engine.device_id())
        .await
        .unwrap();
    assert_eq!(summary.sent, 1);
    assert_eq!(summary.received, 1);
    assert_eq!(titles(&laptop.db).await, ["sinnerman"]);
    assert_eq!(
        collections::playlist(&phone.db, &playlist)
            .await
            .unwrap()
            .unwrap()
            .name,
        "Road trip"
    );

    // Nothing new: nothing moves. The phone can start a sync too.
    let again = phone
        .engine
        .sync_with(laptop.engine.device_id())
        .await
        .unwrap();
    assert_eq!(again, Summary::default());
    let log = sync::sessions(&phone.db, 5).await.unwrap();
    assert_eq!(log.len(), 2, "both sessions are logged on the phone");
    assert!(log.iter().all(|session| session.error.is_none()));
    assert!(
        sync::peers(&phone.db).await.unwrap()[0]
            .last_sync_at
            .is_some()
    );
}

#[tokio::test]
async fn a_conflict_resolves_the_same_on_both_devices() {
    let temp = TempDir::new().unwrap();
    let (phone, laptop) = (device(&temp, "phone").await, device(&temp, "laptop").await);
    pair(&phone, &laptop).await;
    let playlist = collections::create_playlist(&phone.db, "Mix", 1)
        .await
        .unwrap();
    laptop
        .engine
        .sync_with(phone.engine.device_id())
        .await
        .unwrap();

    collections::rename_playlist(&phone.db, &playlist, "Phone's name")
        .await
        .unwrap();
    collections::rename_playlist(&laptop.db, &playlist, "Laptop's name")
        .await
        .unwrap();
    laptop
        .engine
        .sync_with(phone.engine.device_id())
        .await
        .unwrap();

    let name = |db: Arc<Db>| {
        let playlist = playlist.clone();
        async move {
            collections::playlist(&db, &playlist)
                .await
                .unwrap()
                .unwrap()
                .name
        }
    };
    let (on_phone, on_laptop) = (name(phone.db.clone()).await, name(laptop.db.clone()).await);
    assert_eq!(on_phone, on_laptop, "both settle on the newer write");
    assert_eq!(on_laptop, "Laptop's name");
}

#[tokio::test]
async fn changes_reach_a_third_device_through_a_second() {
    let temp = TempDir::new().unwrap();
    let (a, b, c) = (
        device(&temp, "a").await,
        device(&temp, "b").await,
        device(&temp, "c").await,
    );
    pair(&a, &b).await;
    pair(&b, &c).await;
    collections::set_favorite(&a.db, &song("x|from a"), true, 1)
        .await
        .unwrap();

    b.engine.sync_with(a.engine.device_id()).await.unwrap();
    c.engine.sync_with(b.engine.device_id()).await.unwrap();
    assert_eq!(titles(&c.db).await, ["from a"]);
}

#[tokio::test]
async fn a_key_that_does_not_match_the_text_is_refused_before_the_code_is_sent() {
    let temp = TempDir::new().unwrap();
    let (host, joiner) = (device(&temp, "host").await, device(&temp, "joiner").await);
    let offer = host.engine.open_pairing(LOCALHOST).unwrap();
    let mut forged = Offer::parse(&offer).unwrap();
    forged.public_key = Identity::generate().unwrap().public;

    let error = joiner.engine.join(&forged.to_text()).await.unwrap_err();
    assert!(error.to_string().contains("doesn't match"), "{error}");
    assert!(sync::peers(&host.db).await.unwrap().is_empty());
    // The code was never sent, so it still works.
    joiner.engine.join(&offer).await.unwrap();
}

#[tokio::test]
async fn wrong_codes_are_refused_and_lock_the_code() {
    let temp = TempDir::new().unwrap();
    let (host, joiner) = (device(&temp, "host").await, device(&temp, "joiner").await);
    let offer = Offer::parse(&host.engine.open_pairing(LOCALHOST).unwrap()).unwrap();
    let wrong = Offer {
        code: "zzzzzzzz".into(),
        ..offer.clone()
    }
    .to_text();
    for _ in 0..MAX_FAILURES {
        let error = joiner.engine.join(&wrong).await.unwrap_err();
        assert!(error.to_string().contains("wrong"), "{error}");
    }
    let error = joiner.engine.join(&offer.to_text()).await.unwrap_err();
    assert!(
        error.to_string().contains("expired"),
        "cancelled after {MAX_FAILURES} tries"
    );

    let fresh = host.engine.open_pairing(LOCALHOST).unwrap();
    host.clock.fetch_add(CODE_LIFETIME_MS, Ordering::SeqCst);
    let error = joiner.engine.join(&fresh).await.unwrap_err();
    assert!(error.to_string().contains("expired"), "{error}");
}

#[tokio::test]
async fn unpaired_and_removed_devices_cannot_sync() {
    let temp = TempDir::new().unwrap();
    let (phone, laptop, stranger) = (
        device(&temp, "phone").await,
        device(&temp, "laptop").await,
        device(&temp, "stranger").await,
    );
    pair(&phone, &laptop).await;
    collections::set_favorite(&phone.db, &song("x|private"), true, 1)
        .await
        .unwrap();

    // The stranger knows where the phone is and its key, but the phone doesn't know the stranger.
    let mut known = sync::peers(&laptop.db).await.unwrap().remove(0);
    known.paired_at = 1;
    sync::save_peer(&stranger.db, &known).await.unwrap();
    let error = stranger
        .engine
        .sync_with(phone.engine.device_id())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("isn't paired"), "{error}");
    assert!(titles(&stranger.db).await.is_empty());

    // Removing the laptop on the phone cuts it off.
    sync::remove_peer(&phone.db, laptop.engine.device_id())
        .await
        .unwrap();
    assert!(
        laptop
            .engine
            .sync_with(phone.engine.device_id())
            .await
            .is_err()
    );
    assert!(titles(&laptop.db).await.is_empty());
}

#[tokio::test]
async fn only_kinds_both_devices_sync_are_sent() {
    let temp = TempDir::new().unwrap();
    let (phone, laptop) = (device(&temp, "phone").await, device(&temp, "laptop").await);
    pair(&phone, &laptop).await;
    collections::record_play(&phone.db, &song("x|heard"), 5)
        .await
        .unwrap();
    collections::set_favorite(&phone.db, &song("x|loved"), true, 5)
        .await
        .unwrap();
    laptop.engine.set_kinds(vec![SyncKind::Library]);

    laptop
        .engine
        .sync_with(phone.engine.device_id())
        .await
        .unwrap();
    assert_eq!(titles(&laptop.db).await, ["loved"]);
    assert!(
        collections::recent_history(&laptop.db, 5)
            .await
            .unwrap()
            .is_empty()
    );

    // Turning history on sends it after all.
    laptop.engine.set_kinds(SyncKind::ALL.to_vec());
    phone
        .engine
        .sync_with(laptop.engine.device_id())
        .await
        .unwrap();
    laptop
        .engine
        .sync_with(phone.engine.device_id())
        .await
        .unwrap();
    assert_eq!(
        collections::recent_history(&laptop.db, 5)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn large_syncs_are_split_into_batches() {
    let temp = TempDir::new().unwrap();
    let (phone, laptop) = (device(&temp, "phone").await, device(&temp, "laptop").await);
    pair(&phone, &laptop).await;
    let playlist = collections::create_playlist(&phone.db, "Big", 1)
        .await
        .unwrap();
    let tracks: Vec<TrackFields> = (0..1500)
        .map(|n| song(&format!("artist|song {n}")))
        .collect();
    collections::add_to_playlist(&phone.db, &playlist, &tracks, 2)
        .await
        .unwrap();

    let summary = laptop
        .engine
        .sync_with(phone.engine.device_id())
        .await
        .unwrap();
    assert_eq!(summary.received, 1501);
    assert_eq!(
        collections::playlist_items(&laptop.db, &playlist)
            .await
            .unwrap()
            .len(),
        1500
    );
}
