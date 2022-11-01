pub enum NotifyData {
    STOP, // stop any action and exit (send e.g. by CTRL+C)
    NewData,
    NetworkData(Vec<i64>, Vec<i64>),
}
