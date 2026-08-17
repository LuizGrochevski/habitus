use chrono::{Datelike, Duration, NaiveDate};
use std::collections::HashSet;

/// Calcula o streak atual (dias consecutivos) de forma RÍGIDA:
/// se faltar um único dia, o streak quebra e volta a zero a partir dali.
///
/// Regra: o streak conta a partir de "hoje" se hoje já foi feito,
/// ou a partir de "ontem" se hoje ainda não foi feito (dando a chance
/// do dia de hoje ainda não ter acabado). Se nem ontem foi feito, streak = 0.
pub fn current_streak(checkins: &[NaiveDate], today: NaiveDate) -> i32 {
    let done: HashSet<NaiveDate> = checkins.iter().cloned().collect();

    let start = if done.contains(&today) {
        today
    } else if done.contains(&(today - Duration::days(1))) {
        today - Duration::days(1)
    } else {
        return 0;
    };

    let mut streak = 0;
    let mut day = start;
    while done.contains(&day) {
        streak += 1;
        day = day - Duration::days(1);
    }
    streak
}

// Códigos de escape ANSI: instruções invisíveis que o terminal interpreta
// como "pinte o que vem a seguir dessa cor", até encontrar o código de reset.
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_GRAY: &str = "\x1b[90m";
const ANSI_RESET: &str = "\x1b[0m";

/// Gera uma grade de quadrados coloridos (estilo GitHub) representando os
/// últimos `days` dias. Verde = hábito cumprido, cinza = não cumprido.
/// Retorna uma String já formatada em linhas de 7 quadrados (uma "semana" por linha).
pub fn grid(checkins: &[NaiveDate], today: NaiveDate, days: i32) -> String {
    let done: HashSet<NaiveDate> = checkins.iter().cloned().collect();

    let mut squares = Vec::new();
    for i in (0..days).rev() {
        let day = today - Duration::days(i as i64);
        let colored = if done.contains(&day) {
            format!("{}■{}", ANSI_GREEN, ANSI_RESET)
        } else {
            format!("{}■{}", ANSI_GRAY, ANSI_RESET)
        };
        squares.push(colored);
    }

    squares
        .chunks(7)
        .map(|chunk| chunk.join(""))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streak_quebra_com_um_dia_faltando() {
        let today = NaiveDate::from_ymd_opt(2026, 7, 11).unwrap();
        let checkins = vec![
            today - Duration::days(1),
            today - Duration::days(2),
            today - Duration::days(4),
        ];
        assert_eq!(current_streak(&checkins, today), 2);
    }

    #[test]
    fn streak_zero_se_nao_fez_hoje_nem_ontem() {
        let today = NaiveDate::from_ymd_opt(2026, 7, 11).unwrap();
        let checkins = vec![today - Duration::days(2)];
        assert_eq!(current_streak(&checkins, today), 0);
    }

    #[test]
    fn recorde_encontra_maior_sequencia_mesmo_apos_quebra() {
        let today = NaiveDate::from_ymd_opt(2026, 7, 11).unwrap();
        // sequência de 5 dias consecutivos (10 a 6 dias atrás), depois um furo,
        // depois uma sequência mais curta de 2 dias recente. Recorde deve ser 5.
        let checkins = vec![
            today - Duration::days(10),
            today - Duration::days(9),
            today - Duration::days(8),
            today - Duration::days(7),
            today - Duration::days(6),
            // furo aqui
            today - Duration::days(1),
            today,
        ];
        assert_eq!(longest_streak(&checkins), 5);
    }
}

/// Encontra a MAIOR sequência de dias consecutivos em todo o histórico
/// (diferente de `current_streak`, que só olha a sequência mais recente).
/// Serve pra mostrar "seu recorde pessoal", mesmo que o streak atual já
/// tenha quebrado há tempos.
pub fn longest_streak(checkins: &[NaiveDate]) -> i32 {
    if checkins.is_empty() {
        return 0;
    }

    let mut sorted = checkins.to_vec();
    sorted.sort();
    sorted.dedup(); // remove duplicatas, caso existam (não deveria, mas defensivo)

    let mut longest = 1;
    let mut current = 1;

    for window in sorted.windows(2) {
        let gap = (window[1] - window[0]).num_days();
        if gap == 1 {
            current += 1;
        } else {
            current = 1;
        }
        longest = longest.max(current);
    }

    longest
}

/// Agrupa os check-ins por dia e retorna só as datas que bateram (ou passaram)
/// a meta diária. Um hábito com meta 1 (padrão) considera qualquer check-in
/// suficiente; um hábito com meta 3 (ex: "beber água" 3x) só conta o dia como
/// "feito" se aparecerem 3+ check-ins registrados naquele dia.
pub fn qualifying_days(checkins: &[NaiveDate], daily_target: i32) -> Vec<NaiveDate> {
    let mut counts: std::collections::HashMap<NaiveDate, i32> = std::collections::HashMap::new();
    for date in checkins {
        *counts.entry(*date).or_insert(0) += 1;
    }

    counts
        .into_iter()
        .filter(|(_, count)| *count >= daily_target)
        .map(|(date, _)| date)
        .collect()
}

/// Conta quantos check-ins existem numa data específica (usado pra mostrar
/// "X/Y hoje" no comando `done`).
pub fn count_on(checkins: &[NaiveDate], date: NaiveDate) -> i32 {
    checkins.iter().filter(|d| **d == date).count() as i32
}

/// Modo de meta de um hábito: diário (N check-ins NAQUELE dia) ou
/// semanal (N check-ins em QUALQUER dia da mesma semana).
pub enum Frequency {
    Daily { daily_target: i32 },
    Weekly { weekly_target: i32 },
}

/// Retorna a segunda-feira da semana em que `date` cai — usada como
/// "identidade" da semana pra agrupar check-ins.
fn week_start(date: NaiveDate) -> NaiveDate {
    let weekday_from_monday = date.weekday().num_days_from_monday() as i64;
    date - Duration::days(weekday_from_monday)
}

/// Agrupa os check-ins por SEMANA e retorna os inícios de semana (segundas-feiras)
/// que bateram a meta semanal.
pub fn qualifying_weeks(checkins: &[NaiveDate], weekly_target: i32) -> Vec<NaiveDate> {
    let mut counts: std::collections::HashMap<NaiveDate, i32> = std::collections::HashMap::new();
    for date in checkins {
        *counts.entry(week_start(*date)).or_insert(0) += 1;
    }

    counts
        .into_iter()
        .filter(|(_, count)| *count >= weekly_target)
        .map(|(week, _)| week)
        .collect()
}

/// Igual `current_streak`, mas contando SEMANAS consecutivas em vez de dias
/// (o "passo" entre uma semana e a anterior é 7 dias, não 1).
pub fn current_streak_weeks(qualifying_weeks: &[NaiveDate], today: NaiveDate) -> i32 {
    let done: HashSet<NaiveDate> = qualifying_weeks.iter().cloned().collect();
    let this_week = week_start(today);
    let last_week = this_week - Duration::days(7);

    let start = if done.contains(&this_week) {
        this_week
    } else if done.contains(&last_week) {
        last_week
    } else {
        return 0;
    };

    let mut streak = 0;
    let mut week = start;
    while done.contains(&week) {
        streak += 1;
        week = week - Duration::days(7);
    }
    streak
}

/// Igual `longest_streak`, mas pra semanas consecutivas.
pub fn longest_streak_weeks(qualifying_weeks: &[NaiveDate]) -> i32 {
    if qualifying_weeks.is_empty() {
        return 0;
    }

    let mut sorted = qualifying_weeks.to_vec();
    sorted.sort();
    sorted.dedup();

    let mut longest = 1;
    let mut current = 1;

    for window in sorted.windows(2) {
        let gap = (window[1] - window[0]).num_days();
        if gap == 7 {
            current += 1;
        } else {
            current = 1;
        }
        longest = longest.max(current);
    }

    longest
}

/// Ponto de entrada único: calcula (streak atual, recorde) pra um hábito,
/// decidindo automaticamente entre lógica diária ou semanal conforme `freq`.
pub fn progress(checkins: &[NaiveDate], freq: &Frequency, today: NaiveDate) -> (i32, i32) {
    match freq {
        Frequency::Daily { daily_target } => {
            let qualifying = qualifying_days(checkins, *daily_target);
            (
                current_streak(&qualifying, today),
                longest_streak(&qualifying),
            )
        }
        Frequency::Weekly { weekly_target } => {
            let qualifying = qualifying_weeks(checkins, *weekly_target);
            (
                current_streak_weeks(&qualifying, today),
                longest_streak_weeks(&qualifying),
            )
        }
    }
}

#[cfg(test)]
mod frequency_tests {
    use super::*;

    #[test]
    fn semana_bate_meta_com_checkins_espalhados() {
        // segunda, quarta, sexta da mesma semana = 3 check-ins, meta 3
        let monday = NaiveDate::from_ymd_opt(2026, 7, 6).unwrap(); // uma segunda-feira
        let checkins = vec![monday, monday + Duration::days(2), monday + Duration::days(4)];
        let weeks = qualifying_weeks(&checkins, 3);
        assert_eq!(weeks.len(), 1);
        assert_eq!(weeks[0], monday);
    }

    #[test]
    fn streak_semanal_quebra_com_semana_sem_meta() {
        let today = NaiveDate::from_ymd_opt(2026, 7, 13).unwrap(); // segunda-feira
        let last_monday = today - Duration::days(7);
        // só a semana passada bateu meta, essa semana ainda não teve nenhum check-in
        let qualifying = vec![last_monday];
        assert_eq!(current_streak_weeks(&qualifying, today), 1);
    }
}
