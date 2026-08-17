// Testes de integração: vivem fora de src/, só enxergam o que a lib expõe
// como `pub`. Testam o habitus "de fora", como um usuário real (ou outro
// programa, como fizemos no forge) usaria.

use chrono::{Duration, Local, NaiveDate};

use habitus::db;
use habitus::stats;
use habitus::streak;

fn setup_db() -> rusqlite::Connection {
    db::init_db(":memory:").expect("falha ao criar banco em memória")
}

#[test]
fn criar_e_listar_habito() {
    let conn = setup_db();
    db::create_habit(&conn, "ler").unwrap();

    let habits = db::list_habits(&conn).unwrap();
    assert_eq!(habits.len(), 1);
    assert_eq!(habits[0].name, "ler");
}

#[test]
fn multiplos_checkins_no_mesmo_dia_contam_certo() {
    let conn = setup_db();
    db::create_habit(&conn, "beber_agua").unwrap();
    db::set_daily_target(&conn, "beber_agua", 3).unwrap();

    db::mark_done(&conn, "beber_agua").unwrap();
    db::mark_done(&conn, "beber_agua").unwrap();

    let checkins = db::checkins_for(&conn, "beber_agua").unwrap();
    assert_eq!(checkins.len(), 2, "deveria ter 2 check-ins registrados hoje");

    let today = Local::now().date_naive();
    let qualifying = streak::qualifying_days(&checkins, 3);
    assert!(
        !qualifying.contains(&today),
        "não deveria contar como dia batido com só 2/3 check-ins"
    );

    // completa o terceiro
    db::mark_done(&conn, "beber_agua").unwrap();
    let checkins = db::checkins_for(&conn, "beber_agua").unwrap();
    let qualifying = streak::qualifying_days(&checkins, 3);
    assert!(qualifying.contains(&today), "deveria contar com 3/3 check-ins");
}

#[test]
fn undo_remove_apenas_o_ultimo_checkin() {
    let conn = setup_db();
    db::create_habit(&conn, "ler").unwrap();
    db::mark_done(&conn, "ler").unwrap();
    db::mark_done(&conn, "ler").unwrap();

    assert_eq!(db::checkins_for(&conn, "ler").unwrap().len(), 2);

    db::unmark_today(&conn, "ler").unwrap();
    assert_eq!(
        db::checkins_for(&conn, "ler").unwrap().len(),
        1,
        "undo deveria remover só 1 check-in, não todos"
    );
}

#[test]
fn delete_remove_habito_e_checkins() {
    let conn = setup_db();
    db::create_habit(&conn, "ler").unwrap();
    db::mark_done(&conn, "ler").unwrap();

    db::delete_habit(&conn, "ler").unwrap();

    let habits = db::list_habits(&conn).unwrap();
    assert!(habits.is_empty(), "hábito não deveria mais existir");
}

#[test]
fn streak_quebra_com_um_dia_faltando_via_qualifying_days() {
    let today = NaiveDate::from_ymd_opt(2026, 7, 15).unwrap();
    let checkins = vec![
        today - Duration::days(1),
        today - Duration::days(2),
        today - Duration::days(4), // furo em 3 dias atrás
    ];
    let qualifying = streak::qualifying_days(&checkins, 1);
    assert_eq!(streak::current_streak(&qualifying, today), 2);
}

#[test]
fn correlacao_alta_quando_habitos_coincidem_na_maioria_dos_dias() {
    let d1 = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
    let d2 = NaiveDate::from_ymd_opt(2026, 7, 2).unwrap();
    let d3 = NaiveDate::from_ymd_opt(2026, 7, 3).unwrap();

    // treinar: dias 1, 2, 3 | ler: dias 1, 2 -> interseção=2, união=3 -> ~0.67
    let treinar = vec![d1, d2, d3];
    let ler = vec![d1, d2];

    let score = stats::jaccard(&treinar, &ler);
    assert!((score - 0.6667).abs() < 0.01);
}
