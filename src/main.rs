use std::io::{self, Write};
use std::process::Command;

use anyhow::Result;
use chrono::{Local, NaiveDate};
use clap::{Parser, Subcommand};

use habitus::streak::Frequency;
use habitus::{db, stats, streak, tui};

#[derive(Parser)]
#[command(name = "habitus")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Cria um novo hábito
    Habit { name: String },
    /// Marca um hábito como feito hoje
    Done { name: String },
    /// Lista todos os hábitos com seu streak atual
    List,
    /// Mostra o status detalhado de um hábito (streak + grade de dias)
    Status {
        name: String,
        /// Quantidade de dias pra mostrar na grade
        #[arg(long, default_value_t = 28)]
        days: i32,
    },
    /// Remove um hábito e todo o seu histórico de check-ins
    Delete { name: String },
    /// Desmarca o check-in de hoje (corrige um "done" feito por engano)
    Undo { name: String },
    /// Define um horário de lembrete diário pra um hábito (formato HH:MM)
    Remind {
        name: String,
        /// Horário do lembrete, ex: 07:00. Omitir junto com --clear remove o lembrete.
        time: Option<String>,
        /// Remove o lembrete configurado
        #[arg(long)]
        clear: bool,
    },
    /// Checa todos os lembretes e dispara notificação pros que baterem o horário
    /// e ainda não foram feitos hoje. Pensado pra rodar via cron/scheduler, não manualmente.
    CheckReminders,
    /// Define a meta de check-ins diários de um hábito (padrão: 1)
    Target { name: String, value: i32 },
    /// Exporta o histórico de check-ins de um hábito pra CSV
    Export {
        name: String,
        #[arg(long)]
        output: Option<String>,
    },
    /// Mostra um resumo semanal (últimos 7 dias) de todos os hábitos de uma vez
    Week,
    /// Mostra o quanto cada par de hábitos "anda junto" (dias em comum / dias totais)
    Correlate,
    /// Abre o modo TUI com visão geral de todos os hábitos
    Tui,
    /// Define a meta como frequência SEMANAL (ex: 3 = 3x por semana, em
    /// qualquer dia), substituindo a meta diária pra esse hábito
    TargetWeekly {
        name: String,
        value: Option<i32>,
        /// Remove a meta semanal, voltando o hábito pro modo diário
        #[arg(long)]
        clear: bool,
    },
}

/// Checa se `current` está dentro de uma janela de 5 minutos após `target`.
/// A janela existe porque o scheduler que chama `check-reminders` pode não
/// rodar no minuto EXATO — melhor "quase na hora" do que perder o lembrete.
fn within_window(target: &str, current: &str) -> bool {
    fn to_minutes(hhmm: &str) -> Option<i32> {
        let parts: Vec<&str> = hhmm.split(':').collect();
        if parts.len() != 2 {
            return None;
        }
        let h: i32 = parts[0].parse().ok()?;
        let m: i32 = parts[1].parse().ok()?;
        Some(h * 60 + m)
    }

    match (to_minutes(target), to_minutes(current)) {
        (Some(t), Some(c)) => c >= t && c < t + 5,
        _ => false,
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let conn = db::init_db("habitus.db")?;
    let today = Local::now().date_naive();

    match cli.command {
        Commands::Habit { name } => {
            db::create_habit(&conn, &name)?;
            println!("Hábito '{}' criado.", name);
        }
	Commands::Done { name } => {
            db::mark_done(&conn, &name)?;
            let checkins = db::checkins_for(&conn, &name)?;
            let freq = db::habit_frequency(&conn, &name)?;
            let (s, _record) = streak::progress(&checkins, &freq, today);

            match &freq {
                Frequency::Daily { daily_target } => {
                    let count_today = streak::count_on(&checkins, today);
                    if *daily_target > 1 {
                        println!("'{}': {}/{} check-ins hoje.", name, count_today, daily_target);
                    }
                    if count_today >= *daily_target {
                        println!("Meta do dia batida! Streak atual: {} dia(s).", s);
                    } else {
                        println!(
                            "Ainda faltam {} check-in(s) hoje pra bater a meta.",
                            daily_target - count_today
                        );
                    }
                }
                Frequency::Weekly { weekly_target } => {
                    let week_start = today - chrono::Duration::days(
                        chrono::Datelike::weekday(&today).num_days_from_monday() as i64,
                    );
                    let count_this_week = checkins.iter().filter(|d| **d >= week_start).count() as i32;
                    println!(
                        "'{}': {}/{} check-ins essa semana.",
                        name, count_this_week, weekly_target
                    );
                    if count_this_week >= *weekly_target {
                        println!("Meta da semana batida! Streak atual: {} semana(s).", s);
                    } else {
                        println!(
                            "Ainda faltam {} check-in(s) essa semana pra bater a meta.",
                            weekly_target - count_this_week
                        );
                    }
                }
            }
        }
        Commands::List => {
            for h in db::list_habits(&conn)? {
                let checkins = db::checkins_for(&conn, &h.name)?;
                let freq = db::habit_frequency(&conn, &h.name)?;
                let (s, _record) = streak::progress(&checkins, &freq, today);
                let unit = match freq {
                    Frequency::Daily { .. } => "dia(s)",
                    Frequency::Weekly { .. } => "semana(s)",
                };
                println!("- {} (streak: {} {})", h.name, s, unit);
            }
        }
	Commands::Status { name, days } => {
            let checkins = db::checkins_for(&conn, &name)?;
            let freq = db::habit_frequency(&conn, &name)?;
            let (s, record) = streak::progress(&checkins, &freq, today);

            println!("Hábito: {}", name);
            let unit = match &freq {
                Frequency::Daily { daily_target } => {
                    if *daily_target > 1 {
                        println!("Meta diária: {} check-ins", daily_target);
                    }
                    "dia(s)"
                }
                Frequency::Weekly { weekly_target } => {
                    println!("Meta semanal: {} check-ins por semana", weekly_target);
                    "semana(s)"
                }
            };
            println!("Streak atual: {} {}", s, unit);
            println!("Recorde: {} {}\n", record, unit);

            // A grade sempre mostra dias individuais (mesmo pra hábitos de meta
            // semanal), pra você ver ONDE dentro da semana os check-ins caíram.
            let raw_days: Vec<NaiveDate> = match &freq {
                Frequency::Daily { daily_target } => streak::qualifying_days(&checkins, *daily_target),
                Frequency::Weekly { .. } => checkins.iter().cloned().collect(),
            };
            println!("Últimos {} dias:", days);
            println!("{}", streak::grid(&raw_days, today, days));
        }
	Commands::Delete { name } => {
            print!(
                "Tem certeza que quer apagar o hábito '{}' e todo o histórico? (s/N): ",
                name
            );
            io::stdout().flush()?;

            let mut confirmation = String::new();
            io::stdin().read_line(&mut confirmation)?;

            if confirmation.trim().eq_ignore_ascii_case("s") {
                db::delete_habit(&conn, &name)?;
                println!("Hábito '{}' e todo seu histórico foram removidos.", name);
            } else {
                println!("Cancelado. Nada foi apagado.");
            }
        }
	Commands::Undo { name } => {
            db::unmark_today(&conn, &name)?;
            let checkins = db::checkins_for(&conn, &name)?;
            let s = streak::current_streak(&checkins, today);
            println!("'{}' desmarcado pra hoje. Streak atual: {} dia(s).", name, s);
        }
        Commands::Remind { name, time, clear } => {
            if clear {
                db::clear_reminder(&conn, &name)?;
                println!("Lembrete de '{}' removido.", name);
            } else if let Some(t) = time {
                db::set_reminder(&conn, &name, &t)?;
                println!("Lembrete de '{}' definido pra {}.", name, t);
            } else {
                println!("Informe um horário (ex: 'habitus remind treinar 07:00') ou use --clear.");
            }
        }
        Commands::CheckReminders => {
            let now = Local::now();
            let current_time = now.format("%H:%M").to_string();
            let reminders = db::habits_with_reminders(&conn)?;

            for (name, reminder_time) in reminders {
                if !within_window(&reminder_time, &current_time) {
                    continue;
                }

                let checkins = db::checkins_for(&conn, &name)?;
                let already_done = checkins.contains(&today);
                if already_done {
                    continue;
                }

                let _ = Command::new("termux-notification")
                    .arg("--title")
                    .arg("habitus")
                    .arg("--content")
                    .arg(format!("Hora de: {}", name))
                    .status();
            }
        }
	Commands::Target { name, value } => {
            db::set_daily_target(&conn, &name, value)?;
            println!("Meta diária de '{}' definida pra {} check-in(s).", name, value);
        }
        Commands::Export { name, output } => {
            let checkins = db::checkins_for(&conn, &name)?;
            let target = db::daily_target(&conn, &name)?;

            let mut counts: std::collections::HashMap<NaiveDate, i32> = std::collections::HashMap::new();
            for date in &checkins {
                *counts.entry(*date).or_insert(0) += 1;
            }

            let mut dates: Vec<NaiveDate> = counts.keys().cloned().collect();
            dates.sort();

            let path = output.unwrap_or_else(|| format!("{}_checkins.csv", name));
            let mut writer = csv::Writer::from_path(&path)?;
            writer.write_record(["date", "checkins", "daily_target", "target_met"])?;
            for date in dates {
                let count = counts[&date];
                writer.write_record(&[
                    date.to_string(),
                    count.to_string(),
                    target.to_string(),
                    (count >= target).to_string(),
                ])?;
            }
            writer.flush()?;

            println!("Histórico de '{}' exportado pra {}", name, path);
        }
        Commands::Week => {
            for h in db::list_habits(&conn)? {
                let checkins = db::checkins_for(&conn, &h.name)?;
                let freq = db::habit_frequency(&conn, &h.name)?;
                let (s, _record) = streak::progress(&checkins, &freq, today);
                let unit = match freq {
                    Frequency::Daily { .. } => "dia(s)  ",
                    Frequency::Weekly { .. } => "semana(s)",
                };
                let grid = streak::grid(&checkins, today, 7);
                println!("{:15} streak: {:2} {}   {}", h.name, s, unit, grid);
            }
        }
	Commands::Correlate => {
            let habits = db::list_habits(&conn)?;
            if habits.len() < 2 {
                println!("Precisa de pelo menos 2 hábitos cadastrados pra calcular correlação.");
                return Ok(());
            }

            // Pré-calcula os dias "qualificados" (que bateram a meta) de cada hábito uma vez só.
            let mut qualifying_by_habit = Vec::new();
            for h in &habits {
                let checkins = db::checkins_for(&conn, &h.name)?;
                let target = db::daily_target(&conn, &h.name)?;
                qualifying_by_habit.push((h.name.clone(), streak::qualifying_days(&checkins, target)));
            }

            let mut pairs = Vec::new();
            for i in 0..qualifying_by_habit.len() {
                for j in (i + 1)..qualifying_by_habit.len() {
                    let (name_a, days_a) = &qualifying_by_habit[i];
                    let (name_b, days_b) = &qualifying_by_habit[j];
                    let score = stats::jaccard(days_a, days_b);
                    pairs.push((name_a.clone(), name_b.clone(), score));
                }
            }

            pairs.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

            println!("Correlação entre hábitos (dias em comum / dias totais):\n");
            for (a, b, score) in pairs {
                println!("  {} <-> {}: {:.0}%", a, b, score * 100.0);
            }
        }
        Commands::Tui => {
            tui::run(&conn)?;
        }
	Commands::TargetWeekly { name, value, clear } => {
            if clear {
                db::clear_weekly_target(&conn, &name)?;
                println!("Meta semanal de '{}' removida (voltou pro modo diário).", name);
            } else if let Some(v) = value {
                db::set_weekly_target(&conn, &name, v)?;
                println!("Meta de '{}' definida pra {}x por semana.", name, v);
            } else {
                println!("Informe um valor (ex: 'habitus target-weekly leitura 3') ou use --clear.");
            }
        }
	
    }

    Ok(())
}
