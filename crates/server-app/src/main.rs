use server_model::{Command, ReminderFlow};

#[tokio::main]
async fn main() {
    let mut flow = ReminderFlow::new();
    match flow.start() {
        Command::Do(effect) => {
            println!("initial effect requested: {:?}", effect);
        }
        Command::Wait => {
            println!("flow is waiting for external events");
        }
        Command::Done(result) => {
            println!("flow completed immediately: {:?}", result);
        }
    }
}
