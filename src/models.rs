use chrono::NaiveDate;

#[derive(Debug)]
pub struct Habit {
    pub id: i64,
    pub name: String,
}

#[derive(Debug)]
pub struct HabitStatus {
    pub name: String,
    pub current_streak: i32,
    pub checkins: Vec<NaiveDate>, // datas em que o hábito foi cumprido
}
