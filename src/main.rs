use std::io::Read;
use std::io::Write;
use std::net;
use std::io;
use rand::prelude::*;
use dialoguer::Select;

const SIZE: usize = 10; // Maximum 10

#[derive(Copy, Clone, PartialEq, Eq)]
enum EnemyState {
    Unknown,
    Hit,
    Missed
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum OwnState {
    Empty,
    Ship,
    Destroyed
}

fn main() -> io::Result<()> {
    assert!(SIZE <= 10);
    let selection: String = main_menu();
    match selection.as_str() {
        "Host" => server_proc()?,
        "Connect" => client_proc()?,
        _ => println!("Error!")
    }
    Ok(())
}

fn main_menu() -> String {
    println!("
██████╗  █████╗ ████████╗████████╗██╗     ███████╗███████╗██╗  ██╗██╗██████╗ ███████╗
██╔══██╗██╔══██╗╚══██╔══╝╚══██╔══╝██║     ██╔════╝██╔════╝██║  ██║██║██╔══██╗██╔════╝
██████╔╝███████║   ██║      ██║   ██║     █████╗  ███████╗███████║██║██████╔╝███████╗
██╔══██╗██╔══██║   ██║      ██║   ██║     ██╔══╝  ╚════██║██╔══██║██║██╔═══╝ ╚════██║
██████╔╝██║  ██║   ██║      ██║   ███████╗███████╗███████║██║  ██║██║██║     ███████║
╚═════╝ ╚═╝  ╚═╝   ╚═╝      ╚═╝   ╚══════╝╚══════╝╚══════╝╚═╝  ╚═╝╚═╝╚═╝     ╚══════╝                                                                                        
    ");
    let items: Vec<&str> = vec!["Host", "Connect"];
    let selection = Select::new()
        .with_prompt("Do you want to host a game or connect to another player?")
        .items(&items)
        .interact()
        .unwrap();
    return items[selection].to_string();
}

fn server_proc() -> io::Result<()> {
    let port: String = get_input("Enter port of server: ")?;
    let listener: net::TcpListener = net::TcpListener::bind(format!("{}{}", "0.0.0.0:", port))?;
    println!("Listening for connection.");
    let socket: net::TcpStream = listener.accept()?.0;
    println!("Established connection!");
    game_loop(socket, true)?;
    Ok(())
}

fn client_proc() -> io::Result<()> {
    let socket: net::TcpStream = loop {
        let address: String = get_input("Enter IP and port of server: ")?;
        match net::TcpStream::connect(address) {
            Err(e) => {
                println!("Connection failed with the following error:\n{}\nPlease retry.\n", e);
                continue;
            }
            Ok(socket) => break socket
        };
    };
    
    println!("Established connection!");
    game_loop(socket, false)?;
    Ok(())
}

fn game_loop(mut socket: net::TcpStream, turn: bool) -> io::Result<()> {
    let mut own_board: [[OwnState; SIZE]; SIZE] = generate_own_board();
    let mut enemy_board: [[EnemyState; SIZE]; SIZE] = generate_enemy_board();
    println!("Your board:");
    print_own_board(&own_board);
    println!();
    let mut turn: bool = turn;
    let mut turn_counter: u16 = 1;
    let mut cont = true;
    while cont {
        println!("Turn {}:", turn_counter);
        if turn {
            cont = attack(&mut enemy_board, &mut socket)?;
        } else {
            cont = defend(&mut own_board, &mut socket)?;
        }
        turn_counter += 1;
        turn = !turn;
        println!();
        println!("----------------------------------------");
        println!();
    }
    Ok(())
}

fn attack(enemy_board: &mut [[EnemyState; SIZE]; SIZE], socket: &mut net::TcpStream) -> io::Result<bool> {
    println!("Enemy board:");
    print_enemy_board(&enemy_board);
    
    let mut hit_field: String = get_input("Which field would you like to hit? (Format: 6c): ")?;
    while !check_correct_field_format(&hit_field) {
        hit_field = get_input("Wrong format, try again (Format: 6c): ")?;
    }

    // Move coordinates into buffer
    let chars: Vec<char> = hit_field.chars().collect::<Vec<char>>();
    let send_buf: [u8; 2] = [chars[0] as u8, chars[1] as u8];
    socket.write_all(&send_buf)?;

    // Await response (hit?, game over?)
    let mut recv_buf: [u8; 2] = [0 as u8; 2];
    socket.read_exact(&mut recv_buf)?;
    let (row, column) = get_indices_from_field(&hit_field)?;
    let hit: bool = recv_buf[0] as char == 'h';

    if hit {
        println!("Hit! Enemy board now:");
        enemy_board[row][column] = EnemyState::Hit;
    } else {
        println!("No hit! Enemy board now:");
        enemy_board[row][column] = EnemyState::Missed;
    }
    print_enemy_board(&enemy_board);
    
    let game_over: bool = recv_buf[1] as char == 'o';
    if game_over {
        println!("You won!");
        return Ok(false);
    }
    Ok(true)
}

fn defend(own_board: &mut [[OwnState; SIZE]; SIZE], socket: &mut net::TcpStream) -> io::Result<bool> {
    println!("Wait for your opponent to choose a field.");
    let mut recv_buf: [u8; 2] = [0 as u8; 2];
    socket.read_exact(&mut recv_buf)?; // receive coordinates of attempted hit

    // Reconstruct coordinate string from buffer
    let mut hit_field: String = String::new();
    hit_field.push(recv_buf[0] as char);
    hit_field.push(recv_buf[1] as char);

    let (row, column) = get_indices_from_field(&hit_field)?;
    let hit: bool = own_board[row][column] != OwnState::Empty;
    println!("{}", if hit { "The enemy hit you!" } else { "The enemy did not hit you!" });
    if hit { 
        own_board[row][column] = OwnState::Destroyed;
        println!("Your board now:");
        print_own_board(&own_board);
    }
    
    // Check if game is over
    let mut over: bool = true;
    for row in 0..SIZE {
        for column in 0..SIZE {
            if own_board[row][column] == OwnState::Ship { over = false; }
        }
    }

    let result: char = if hit { 'h' } else { 'm' }; // hit, miss
    let game_state: char = if over { 'o' } else { 'n' }; // over, not over
    let send_buf: [u8; 2] = [result as u8, game_state as u8]; // send whether the hit was successful and whether the game is over
    socket.write_all(&send_buf)?;

    if over {
        println!("Game over! You lost.");
        return Ok(false);
    }
    Ok(true)
}

fn check_correct_field_format(field: &str) -> bool {
    let chars: Vec<char> = field.chars().collect::<Vec<char>>();
    if chars.len() != 2 { return false; }
    if !char::is_alphanumeric(chars[0]) || !char::is_alphabetic(chars[1]) {
        return false;
    }
    return true;
}

fn get_indices_from_field(field: &str) -> io::Result<(usize, usize)> {
    let chars: Vec<char> = field.chars().collect::<Vec<char>>();
    let row_index: usize = chars[0] as usize - '0' as usize;
    let column_index: usize = chars[1].to_ascii_lowercase() as usize - 'a' as usize;
    Ok((row_index, column_index))
}

fn print_own_board(board: &[[OwnState; SIZE]; SIZE]) {
    print_header();

    // Print actual board
    for row in 0..SIZE {
        print!("{row} ");
        for column in 0..SIZE {
            let char: char = match board[row][column] {
                OwnState::Empty => '◦',
                OwnState::Destroyed => '□',
                OwnState::Ship => '■'
            };
            print!("{} ", char);
        }
        println!();
    }
}

fn print_enemy_board(board: &[[EnemyState; SIZE]; SIZE]) {
    print_header();

    // Print actual board
    for row in 0..SIZE {
        print!("{row} ");
        for column in 0..SIZE {
            let char: char = match board[row][column] {
                EnemyState::Unknown => '◦',
                EnemyState::Missed => '□',
                EnemyState::Hit => '■'
            };
            print!("{} ", char);
        }
        println!();
    }
}

fn print_header() {
    print!("  ");
    for column in 0..SIZE {
        print!("{} ", (b'A' + column as u8) as char);
    }
    println!();
}

fn generate_own_board() -> [[OwnState; SIZE]; SIZE] {
    let mut own_board: [[OwnState; SIZE]; SIZE] = [[OwnState::Empty; SIZE]; SIZE];
    let num_boats: usize = SIZE / 2;
    let mut rng = thread_rng();
    // Each boat has a different length, loop through each boat
    let mut boat_len: usize = 2;
    while boat_len <= num_boats {
        let horizontal: bool = rng.gen_bool(0.5);

        // Determine coordinates for spawn point of boat
        let mut row: usize = rng.gen_range(0..SIZE);
        let mut column: usize = rng.gen_range(0..SIZE);

        // Make sure the spawn point is valid and the boat will fit
        if horizontal {
            if column + boat_len >= SIZE { column -= column + boat_len - SIZE };
        } else {
            if row + boat_len >= SIZE { row -= row + boat_len - SIZE };
        }

        // Check if boat would intersect other boat, if so, retry spawn
        let mut possible = true;
        for i in 0..boat_len {
            if horizontal {
                if own_board[row][column + i] == OwnState::Ship { possible = false }
            } else {
                if own_board[row + i][column] == OwnState::Ship { possible = false }
            }
        }

        if possible {
            for i in 0..boat_len {
                if horizontal {
                    own_board[row][column + i] = OwnState::Ship;
                } else {
                    own_board[row + i][column] = OwnState::Ship;
                }
            }
            boat_len += 1;
        }
    }
    own_board
}

fn generate_enemy_board() -> [[EnemyState; SIZE]; SIZE] {
    let enemy_board: [[EnemyState; SIZE]; SIZE] = [[EnemyState::Unknown; SIZE]; SIZE];
    enemy_board
}

fn get_input(prompt: &str) -> io::Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut input: String = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_string();
    Ok(input)
}