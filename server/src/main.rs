use std::thread;
use std::time::Duration;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::sync::mpsc;
// use std::sync::mpsc::{ RecvTimeoutError, TryRecvError };
use std::sync::{ Arc, Mutex };
extern crate sysinfo;
// use sysinfo::{ System, SystemExt };
use std::fs::File;
use std::collections::HashMap;
use machine_info::Machine;
use std::env;
use std::process;
use std::io::{ self, Read, Write };
use termion::color;
use image::{ DynamicImage, GenericImageView, ImageBuffer, ImageResult, Rgba };
use std::path::Path;
use bincode::{ serialize, deserialize };


const BUFFER_SIZE: usize = 65507; // Maximum UDP datagram size
const END_MARKER: &[u8] = b"END_OF_IMAGE"; // Define the end marker


#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct pstRow {
   id: usize,
   CPU_utilization: i32,
}


impl pstRow {
   fn new(id: usize, CPU_utilization: i32) -> pstRow {
       pstRow {
           id,
           CPU_utilization,
       }
   }
}


fn parse_server_message(input_string: &str) -> (String, u8, i32) {
   let mess_type: u8 = input_string.as_bytes()[0] - 48;


   // Convert the next 8 bits to u8 (sender ID)
   let sid: u8 = input_string.as_bytes()[1] - 48;


   // Extract the rest of the string (from index 2 to the end)
   let mut utilization: &str = &input_string[2..];


   let mut cpu_utilization: i32 = utilization.parse().unwrap();


   // Print the extracted values
   // println!("Message Type ID: {}", mess_type);
   // println!("Sender ID: {}", sid);
   // println!("Rest of the string: {}", cpu_utilization);


   (mess_type.to_string(), sid, cpu_utilization)
}


fn is_image_complete(chunks: &[Vec<u8>]) -> bool {
   // Check if the end marker exists at the end of the received data
   if let Some(last_chunk) = chunks.last() {
       last_chunk.ends_with(END_MARKER)
   } else {
       false
   }
}


fn embed_image(cover_image: DynamicImage, hidden_image: DynamicImage) -> DynamicImage {
   let (width, height) = cover_image.dimensions();
   let hidden_image = hidden_image.resize_exact(
       width,
       height,
       image::imageops::FilterType::Lanczos3
   );


   let mut cover_buffer = cover_image.to_rgba8();
   let hidden_buffer = hidden_image.to_rgba8();


   for (x, y, cover_pixel) in cover_buffer.enumerate_pixels_mut() {
       let hidden_pixel = hidden_buffer.get_pixel(x, y);


       let (r, _g, _b, a) = (cover_pixel[0], cover_pixel[1], cover_pixel[2], cover_pixel[3]);
       let (hr, hg, hb, ha) = (hidden_pixel[0], hidden_pixel[1], hidden_pixel[2], hidden_pixel[3]);


       cover_pixel[0] = (r & 0xf0) | (hr >> 4);
       cover_pixel[1] = (_g & 0xf0) | (hg >> 4);
       cover_pixel[2] = (_b & 0xf0) | (hb >> 4);
       //    cover_pixel[3] = 0;
       //    let new_alpha = ((a as u16 * (255 - transparency as u16)) / 255) as u8;
       //         cover_pixel[3] = new_alpha;
       let new_alpha = (((ha as u16) * (255 - (100 as u16))) / 255) as u8;
       cover_pixel[3] = a.saturating_add(new_alpha);
   }


   DynamicImage::ImageRgba8(cover_buffer)
}


fn main() {
   //format of the cpu utilization message is 0, id, and utilization
   //format of the token message is 1, sid, 0
   let args: Vec<String> = env::args().collect();
   let program_name = &args[0];
   println!("Program name: {}", program_name);


   if args.len() < 3 {
       println!("Invalid Server Index!");
       println!("Command Format: cargo run <server_index> <simulate_recovery_option>");
       println!(
           "To simulate recovery put 1 for <simulate_recovery_option> and 0 to not simulate recovery"
       );
       process::exit(0);
   }


   let index = args[1].parse::<usize>().unwrap();
   if index >= 0 && index <= 2 {
       println!("Index: {}", index);
   } else {
       println!("Invalid Server Index!");
       println!("Command Format: cargo run <server_index> <simulate_recovery_option>");
       println!(
           "To simulate recovery put 1 for <simulate_recovery_option> and 0 to not simulate recovery"
       );
       process::exit(0);
   }


   let sim_recovery: bool;
   let third_arg = args[2].parse::<usize>().unwrap();
   if third_arg == 1 {
       sim_recovery = true;
   } else if third_arg == 0 {
       sim_recovery = false;
   } else {
       println!("Invalid Option!");
       println!("Command Format: cargo run <server_index> <simulate_recovery_option>");
       println!(
           "To simulate recovery put 1 for <simulate_recovery_option> and 0 to not simulate recovery"
       );
       process::exit(0);
   }


   let my_idx = index;
   let all_ips = ["127.0.0.1", "127.0.0.2", "127.0.0.3"];
   let my_ip = all_ips[my_idx as usize];


   let mut my_requests = Arc::new(Mutex::new(0));
   let my_requests_clone = Arc::clone(&my_requests);
   let my_requests_clone2 = Arc::clone(&my_requests);
   let my_requests_clone3 = Arc::clone(&my_requests);


   let (lw_serv_mess_tx, lw_serv_mess_rx) = mpsc::channel();
   // let (do_encrypt_tx, do_encrypt_rx) = mpsc::channel();
   let (token_tx, token_rx) = mpsc::channel();
   let (send_accept_tx, send_accept_rx) = mpsc::channel();
   let (img_tx, img_rx) = mpsc::channel();
   let (waiting_img_tx, waiting_img_rx) = mpsc::channel();
   let (encrypted_img_tx, encrypted_img_rx) = mpsc::channel();
   let (lw_dos_tx, lw_dos_rx) = mpsc::channel();
   let (ws_dos_entry_tx, ws_dos_entry_rx) = mpsc::channel();
   let (ws_dos_map_tx, ws_dos_map_rx) = mpsc::channel();


   let mut pst_data: Vec<pstRow> = [
       pstRow { id: 0, CPU_utilization: 3 },
       pstRow { id: 1, CPU_utilization: 2 },
       pstRow { id: 2, CPU_utilization: 1 },
   ].to_vec();
   pst_data.sort_by(|a, b| a.CPU_utilization.cmp(&b.CPU_utilization));
   let pst = Arc::new(Mutex::new(pst_data));
   let pst_clone = Arc::clone(&pst);
   let pst_clone2 = Arc::clone(&pst);


   //SERVER THREADS


   // 1. Listener thread
   thread::spawn(move || {
       //Listen to message
       // println!("Hola");
       let lis_socket = Arc::new(
           UdpSocket::bind(format!("{}:8083", my_ip)).expect("Failed to bind to address")
       );
       let mut buf = [0u8; 1024];
       loop {
           match lis_socket.recv_from(&mut buf) {
               Ok((n, src)) => {
                   let received_message = String::from_utf8_lossy(&buf[0..n]).to_string();
                   println!("\nReceived {} bytes from {} = {}", n, src, received_message);
                   // println!("TEST: {}", src.to_string());
                   lw_serv_mess_tx.send(received_message).unwrap();
                   // println!("Sent through channel");
               }
               Err(e) => {
                   eprintln!("Error receiving data: {}", e);
               }
           }
       }
   });


   // 2. Worker thread
   thread::spawn(move || {
       //Parse message and update PST.
       //After 20 seconds, send token


       loop {
           // println!("Waiting to receive from the channel the recevied message");
           let received_message = lw_serv_mess_rx.recv().unwrap();
           let (opcode, sid, rec_util) = parse_server_message(&received_message);


           //This is a CPU utilization message
           if opcode == "0" {
               // update pst
               let pos = pst_clone
                   .lock()
                   .unwrap()
                   .iter()
                   .position(|&r| r.id == (sid as usize))
                   .unwrap();
               pst_clone.lock().unwrap()[pos].CPU_utilization = rec_util;
               pst_clone
                   .lock()
                   .unwrap()
                   .sort_by(|a, b| a.CPU_utilization.cmp(&b.CPU_utilization));
               token_tx.send(false).unwrap();
           } else if
               //This is a token message
               opcode == "1"
           {
               //I have received a token, so check If I have any number of requests
               token_tx.send(true).unwrap();
               // if *my_requests.lock().unwrap() == 0 {
               //     //send cpu utilization with infinite value because i will fail
               //     infinite_util_tx.send(true).unwrap();
               // } else {
               //     //send token to next server
               //     infinite_util_tx.send(false).unwrap();
               // }
           }
           println!("PST: {:?}", pst_clone.lock().unwrap());
       }
   });


   // 3. Sender Thread
   thread::spawn(move || {
       let send_socket = Arc::new(
           UdpSocket::bind(format!("{}:8082", my_ip)).expect("Failed to bind to address")
       );
       let sender_socket = Arc::new(Mutex::new(send_socket));
       let sender_socket_clone = Arc::clone(&sender_socket);
       let sender_socket_clone2 = Arc::clone(&sender_socket);


       let m = Arc::new(Mutex::new(Machine::new()));
       let m_clone = Arc::clone(&m);
       let m_clone2 = Arc::clone(&m);


       let cpu_util = Arc::new(Mutex::new(150));
       let cpu_util_clone = Arc::clone(&cpu_util);
       let cpu_util_clone2 = Arc::clone(&cpu_util);


       let mut have_token = false;
       let mut first_token = false;
       if my_idx == 0 && sim_recovery {
           first_token = true;
           have_token = true;
       }


       // let mut cpu_flag = false;
       let mut cpu_flag = Arc::new(Mutex::new(false));
       let mut cpu_flag_clone = Arc::clone(&cpu_flag);
       let mut cpu_flag_clone2 = Arc::clone(&cpu_flag);


       thread::spawn(move || {
           // Multicast to all servers my cpu utilization
           loop {
               // if *my_requests.lock().unwrap() == 0 {
               //     cpu_flag.lock().unwrap() = true;
               // }
               print!("{}", color::Fg(color::LightRed));
               // println!("CPU Flag in Utilization Thread {}", cpu_flag_clone.lock().unwrap());
               print!("{}", color::Fg(color::Reset));
               io::stdout().flush().unwrap();


               if !cpu_flag_clone.lock().unwrap().clone() && !first_token {
                   // println!("AM I HERE?");
                   *cpu_util_clone.lock().unwrap() = m_clone
                       .lock()
                       .unwrap()
                       .system_status()
                       .unwrap().cpu;
               }
               // else {
               //     *cpu_util_clone.lock().unwrap() = 150;
               // }
               print!("{}", color::Fg(color::LightYellow));
               println!("Sending a utilization of: {}", cpu_util_clone.lock().unwrap());
               print!("{}", color::Fg(color::Reset));
               io::stdout().flush().unwrap();


               for ip in all_ips {
                   // if ip != my_ip {
                   let socket_address = ip.to_owned() + ":8083";
                   let msg = format!("0{}{}", my_idx, cpu_util_clone.lock().unwrap());
                   let _ = sender_socket_clone
                       .lock()
                       .unwrap()
                       .send_to(msg.as_bytes(), socket_address);
                   // }
               }
               thread::sleep(Duration::from_secs(5));
           }
       });


       //Sending TOKEN Thread
       thread::spawn(move || {
           loop {
               if !first_token {
                   match token_rx.recv() {
                       Ok(message) => {
                           println!("Received Answer to DO I HAVE A TOKEN?: {}", message);
                           have_token = message;
                       }
                       Err(_) => {}
                   }
                   if have_token && *my_requests_clone.lock().unwrap() == 0 {
                       *cpu_flag_clone2.lock().unwrap() = true;
                   }
               }


               print!("{}", color::Fg(color::LightCyan));
               // println!("CPU Flag in TOKEN Thread {}", cpu_flag_clone2.lock().unwrap());
               print!("{}", color::Fg(color::Reset));
               io::stdout().flush().unwrap();


               // println!("DO I HAVE A TOKEN HERE? {}", have_token);
               if have_token {
                   if *cpu_flag_clone2.lock().unwrap() || first_token {
                       println!("I should fail soon");
                       // println!("CPU Flag in Token Thread {}", cpu_flag_clone2.lock().unwrap());
                       *cpu_util_clone2.lock().unwrap() = 150;
                       let msg = format!("0{}{}", my_idx, cpu_util_clone2.lock().unwrap());
                       println!("MY MSG: {}", msg);
                       for ip in all_ips {
                           if ip != my_ip {
                               let socket_address = ip.to_owned() + ":8083";
                               let _ = sender_socket_clone2
                                   .lock()
                                   .unwrap()
                                   .send_to(msg.as_bytes(), socket_address);
                           }
                       }


                       print!("{}", color::Fg(color::Red));
                       println!("I have Failed!");
                       print!("{}", color::Fg(color::Reset));
                       io::stdout().flush().unwrap();


                       thread::sleep(Duration::from_secs(20));


                       print!("{}", color::Fg(color::Green));
                       println!("I have Revived!");
                       print!("{}", color::Fg(color::Reset));
                       io::stdout().flush().unwrap();


                       *cpu_util_clone2.lock().unwrap() = m_clone2
                           .lock()
                           .unwrap()
                           .system_status()
                           .unwrap().cpu;
                       *cpu_flag_clone2.lock().unwrap() = false;
                       first_token = false;
                       thread::sleep(Duration::from_secs(10));
                       let socket_address = all_ips[(my_idx + 1) % 3].to_owned() + ":8083";
                       let _ = sender_socket_clone2
                           .lock()
                           .unwrap()
                           .send_to(format!("1{}0", my_idx).as_bytes(), socket_address);


                       print!("{}", color::Fg(color::LightGreen));
                       println!("Sent token bcz i have revived");
                       print!("{}", color::Fg(color::Reset));
                       io::stdout().flush().unwrap();


                       have_token = false;
                   } else {
                       let socket_address = all_ips[(my_idx + 1) % 3].to_owned() + ":8083";
                       let _ = sender_socket_clone2
                           .lock()
                           .unwrap()
                           .send_to(format!("1{}0", my_idx).as_bytes(), socket_address);


                       print!("{}", color::Fg(color::LightRed));
                       println!("Passed token bcz im busy");
                       print!("{}", color::Fg(color::Reset));
                       io::stdout().flush().unwrap();


                       process::exit(0);
                   }
               }
           }
       });
   });


   // ************* CLIENT THREADS ****************


   //Client sends a message before the actual request. if message is "0" then DoS Service.
   //If message "2" then Encryption Service


   let mut online_users_mutex: Arc<Mutex<HashMap<String, SocketAddr>>> = Arc::new(
       Mutex::new(HashMap::new())
   );
   let online_users_clone = online_users_mutex.clone();
   let online_users_clone2 = online_users_mutex.clone();


   // 4. Listener thread to Clients
   thread::spawn(move || {
       //Listen to messages sent from clients to servers
       //it will not only listen but also checks whether to accept or not
       let lis_socket = Arc::new(
           UdpSocket::bind(format!("{}:8086", my_ip)).expect("Failed to bind to address")
       );
       let mut buf = [0u8; 65507];
       let mut waiting_img = false;
       loop {
           match lis_socket.recv_from(&mut buf) {
               Ok((n, src)) => {
                   print!("\x1B[1m"); // ANSI escape code for bold text
                   print!("\x1B[4m"); // ANSI escape code for underlined text
                   print!("{}", color::Fg(color::LightRed));
                   println!("Received New Client Request!");
                   print!("{}", color::Fg(color::Reset));
                   print!("\x1B[0m"); // ANSI escape code to reset formatting
                   io::stdout().flush().unwrap();
                   let received_message = String::from_utf8_lossy(&buf[0..n]).to_string();
                   let image_data = buf[..n].to_vec();
                   let parts: Vec<&str> = received_message.split(',').collect();
                   println!("Received {} bytes from {}: {}", n, src, received_message);
                   println!(
                       "THE waiting for image bool is {} and the leader is {}",
                       waiting_img,
                       pst_clone2.lock().unwrap()[0].id
                   );


                   if pst_clone2.lock().unwrap()[0].id == my_idx && !waiting_img {
                       println!("I am the leader; therefore I accept the requested workload");
                       send_accept_tx.send(("ACK".as_bytes(), src.clone())).unwrap();
                       println!("The RECEIVED MESSAGE IS {}", received_message);
                       // thread::sleep(Duration::from_secs(6));
                       // assert_eq!(received_message, "2");
                       if received_message == "2" {
                           println!("Entered the mini if statement");
                           waiting_img = true;
                           let requests_ctr = *my_requests_clone2.lock().unwrap();
                           *my_requests_clone2.lock().unwrap() = requests_ctr + 1;
                           println!("Entered the mini if statement 2");
                       } else {
                           //the requested service is not encryption


                           if parts.len() == 2 && parts[0] == "LOGIN" {
                               println!("I know that it is a LOGIN");
                               let username = parts[1];
                               lw_dos_tx
                                   .send(("LOGIN", username.to_string(), src.clone()))
                                   .unwrap();
                           } else if parts.len() == 2 && parts[0] == "LOGOUT" {
                               let username = parts[1];
                               lw_dos_tx
                                   .send(("LOGOUT", username.to_string(), src.clone()))
                                   .unwrap();
                           } else {
                               // Drop the message and do nothing --> I am not the leader.
                               println!("Client is requesting an unsupported service");
                           }
                       }
                   } else if waiting_img {
                       print!("\x1B[1m"); // ANSI escape code for bold text
                       print!("\x1B[4m"); // ANSI escape code for underlined text
                       print!("{}", color::Fg(color::LightRed));
                       println!("ABOUT TO SEND THE IMAGE THROUGH THE CHANNEL: {:?}", image_data);
                       print!("{}", color::Fg(color::Reset));
                       print!("\x1B[0m"); // ANSI escape code to reset formatting
                       io::stdout().flush().unwrap();
                       img_tx.send((src, image_data)).unwrap();
                       thread::sleep(Duration::from_secs(1));
                       println!("I HAVE SENT THE IMAGE THROUGH THE CHANNEL");
                       match waiting_img_rx.try_recv() {
                           Ok(message) => {
                               println!("Still Waiting Image?: {}", message);
                               waiting_img = message;
                           }
                           Err(_) => println!("No waiting message received"),
                       }
                       thread::sleep(Duration::from_secs(1));
                       buf = [0u8; 65507];
                   } else {
                       println!("I am not the leader therefore, the request is dropped");
                       // Drop the message and do nothing --> I am not the leader.
                   }
               }
               Err(e) => {
                   eprintln!("Error receiving data: {}", e);
               }
           }
       }
   });


   // 5. Worker thread (Image Encryption)
   thread::spawn(move || {
       //Create a buffer to buffer requests **************************************
       let mut counter = 0;
       let chunks_map: Arc<Mutex<HashMap<SocketAddr, Vec<Vec<u8>>>>> = Arc::new(
           Mutex::new(HashMap::new())
       );


       loop {
           match img_rx.recv() {
               Ok((src, received_image_data)) => {
                   print!("\x1B[1m"); // ANSI escape code for bold text
                   print!("\x1B[4m"); // ANSI escape code for underlined text
                   print!("{}", color::Fg(color::LightGreen));
                   println!("INSIDE THE FOR LOOP: {}, {:?}", src, received_image_data);
                   print!("{}", color::Fg(color::Reset));
                   print!("\x1B[0m"); // ANSI escape code to reset formatting
                   io::stdout().flush().unwrap();
                   counter += 1;


                   print!("\x1B[1m"); // ANSI escape code for bold text
                   print!("\x1B[4m"); // ANSI escape code for underlined text
                   print!("{}", color::Fg(color::LightGreen));
                   println!("in the worker loop");
                   print!("{}", color::Fg(color::Reset));
                   print!("\x1B[0m"); // ANSI escape code to reset formatting
                   io::stdout().flush().unwrap();


                   println!("Src: {}", src);
                   // Assemble chunks for each client
                   let mut chunks_map = chunks_map.lock().unwrap();
                   let client_chunks: &mut Vec<Vec<u8>> = chunks_map
                       .entry(src)
                       .or_insert(Vec::new());
                   client_chunks.push(received_image_data.clone());


                   // print!("\x1B[1m"); // ANSI escape code for bold text
                   // print!("\x1B[4m"); // ANSI escape code for underlined text
                   // print!("{}", color::Fg(color::LightCyan));
                   // println!("Client Chunks: {:?}", client_chunks);
                   // print!("{}", color::Fg(color::Reset));
                   // print!("\x1B[0m"); // ANSI escape code to reset formatting
                   // io::stdout().flush().unwrap();


                   // Check if the image is complete
                   let byte_size = received_image_data.clone().len();
                   // Check if the image is complete
                   if byte_size < 65507 {
                       // Save the received data as an image file
                       let image_filename = format!(
                           "my_image.jpeg",
                           //chrono::Utc::now().timestamp()
                       );
                       let mut file = File::create(&image_filename).unwrap();
                       for chunk in &mut *client_chunks {
                           file.write_all(&chunk).unwrap();
                       }
                       file.sync_all().unwrap();
                       println!("Image received and saved as {}.", image_filename);
                       client_chunks.clear();


                       //let filename_clone = image_filename.clone();


                       //REMOVE THE FOLLOWING 4 Lines
                       // let mut file = File::create(image_filename).unwrap();
                       // for chunk in &mut *client_chunks {
                       //     // println!("I am going to write in the file now!");
                       //     if chunk != END_MARKER {
                       //         println!("The chunk {:?}", chunk);
                       //         file.write_all(&chunk).unwrap();
                       //     }
                       // }


                       //println!("Image received and saved as {}.", filename_clone);
                       waiting_img_tx.send(false).unwrap();


                       // Clear the chunks for this client
                       // client_chunks.clear();
                       counter = 0;


                       //Encrypt the image
                       //if cover image doesn't exist print image doesn't exist
                       let mut cover_image = match image::open("cover.png") {
                           Ok(img) => img,
                           Err(err) => {
                               eprintln!("Error opening image: {:?}", err);
                               // Handle the error appropriately (return, panic, etc.)
                               // For now, let's panic to stop the execution
                               panic!("Unable to open image");
                           }
                       };


                       let hidden_image = image::open("my_image.jpeg").unwrap();


                       let (hidden_width, hidden_height) = hidden_image.dimensions();
                       cover_image = cover_image.resize_exact(
                           hidden_width,
                           hidden_height,
                           image::imageops::FilterType::Lanczos3
                       );


                       let embedded_image = embed_image(cover_image.clone(), hidden_image);


                       //Remove the following 2 lines
                       let output_path = Path::new("output.png");
                       embedded_image.save(output_path).unwrap();


                       encrypted_img_tx.send((src, output_path)).unwrap();
                       drop(chunks_map);
                   } else {
                       println!("not done: {}", counter);
                   }
               }
               Err(_) => {}
           }
           // println!("out of receiving loop");
       }
   });


   // 6. Worker thread (DoS)
   thread::spawn(move || {
       // Receive data from the dos_rx channel and process it


       loop {
           match lw_dos_rx.try_recv() {
               Ok((operation, username, src)) => {
                   if operation == "LOGIN" {
                       online_users_clone.lock().unwrap().insert(username.clone(), src);
                       println!("User {} logged in.", username);
                       // let response = ("LOGIN_SUCCESS".to_string(), src, online_users.clone());
                       // let entry_response = ("LOGIN_SUCCESS".to_string(), (username, src));
                       // let entry_response = format!("LOGIN_SUCCESS,{},{}", username, src);
                       // let entry_response = format!("MAP,{},{}", username, src);
                       let map_response = format!(
                           "LOGIN_SUCCESS,{:?}",
                           online_users_clone.lock().unwrap().clone()
                       ); // Send the whole DOS
                       println!("Map_Response: {:?}", map_response);
                       let entry_response = format!("ENTRY,{},{}", username, src); // Send only an entry of the DOS
                       ws_dos_entry_tx.send((entry_response, src)).unwrap();
                       ws_dos_map_tx.send((map_response, src)).unwrap();
                   } else if operation == "LOGOUT" {
                       if let Some(_) = online_users_clone.lock().unwrap().remove(&username) {
                           println!("User {} logged out.", username);
                           // Respond with a success message or any other response if needed
                           // let entry_response = ("LOGOUT_SUCCESS".to_string(), (username, src));
                           let entry_response = format!(
                               "LOGOUT_SUCCESS,{},{}",
                               username.clone(),
                               src
                           ); // Send only an entry of the DOS
                           ws_dos_entry_tx.send((entry_response, src)).unwrap();
                       } else {
                           // Respond with an error message if the user was not found
                           // let response = ("LOGOUT_FAILURE".to_string(), (username, src));
                           let response = format!("LOGOUT_FAILURE,{},{}", username, src); // Send the whole DOS
                           ws_dos_entry_tx.send((response, src)).unwrap();
                       }
                   } else {
                       println!("Invalid operation");
                   }


                   // let _ = sender_socket.lock().unwrap().send_to(msg, dest_socket_addr);
                   // println!("Sent: {:?}", msg);
                   // println!("To: {:?}", dest_socket_addr);
               }
               Err(_) => {}
           }
       }
   });


   // 7. Sender Thread
   thread::spawn(move || {
       let send_socket = UdpSocket::bind(format!("{}:8085", my_ip)).expect(
           "Failed to bind to address"
       );
       let sender_socket = Arc::new(Mutex::new(send_socket));
       // let sender_socket_clone = Arc::clone(&sender_socket);
       // let sender_socket_clone2 = Arc::clone(&sender_socket);


       loop {
           match send_accept_rx.try_recv() {
               Ok((msg, dest_socket_addr)) => {
                   let _ = sender_socket.lock().unwrap().send_to(msg, dest_socket_addr);
                   println!("Sent: {:?}", msg);
                   println!("To: {:?}", dest_socket_addr);
               }
               Err(_) => {}
           }


           // SENDING DOS SERVICE REPLIES


           // let entry_response = ("LOGIN_SUCCESS".to_string(), (username, src));
           // let map_response = (online_users.clone(), src);


           // let map_response = format!("LOGIN_SUCCESS,{:?}", online_users.clone()); // Send the whole DOS
           // let entry_response = format!("ENTRY,{},{}", username, src); // Send only an


           // Entry Updates
           match ws_dos_entry_rx.try_recv() {
               Ok((message, src)) => {
                   // let serialized_data = serialize(&(msg.clone(), (username.clone(), src))).expect(
                   //     "Failed to serialize tuple"
                   // );
                   // let entry_message = format!("ENTRY,{:?}", serialized_data);
                   // TODO: Send to all clients the entry except the sender client
                   for client in online_users_clone2.lock().unwrap().values() {
                       if client.ip().to_string() != src.ip().to_string() {
                        // println!()
                           let _ = sender_socket
                               .lock()
                               .unwrap()
                               .send_to(
                                   message.as_bytes(),
                                   client.ip().to_string().to_owned() + ":9091"
                               );
                           println!("Sent: {:?}", message);
                           println!("To: {:?}", client.ip());
                       }
                   }
               }
               Err(_) => {}
           }


           // Sending the whole DOS for new clients
           match ws_dos_map_rx.try_recv() {
               Ok((message, src)) => {
                   // let serialized_data = serialize(&users_map).expect("Failed to serialize map");
                   // let map_message = format!("MAP,{:?}", serialized_data);
                   let _ = sender_socket
                       .lock()
                       .unwrap()
                       .send_to(message.as_bytes(), src.ip().to_string().to_owned() + ":9091");
                   println!("Sent: {:?}", message);
                   println!("To: {:?}", src);
               }
               Err(_) => {}
           }


           //match receiving the encrypted image to be sent back to the client
           match encrypted_img_rx.try_recv() {
               Ok((dest_socket_addr, file_path)) => {
                   let mut file = File::open(file_path).unwrap();
                   let img_size = std::fs::metadata(file_path).unwrap().len();
                   let chunks = (img_size as f64) / (BUFFER_SIZE as f64);
                   // Buffer to hold each chunk of data


                   // Loop to read and send chunks of data
                   for chunk_number in 0..chunks.ceil() as i32 {
                       let mut buffer = [0u8; BUFFER_SIZE];
                       let bytes_read = file.read(&mut buffer).unwrap();
                       println!("Bytes Read: {:?}", bytes_read);


                       // Send the current chunk of data to the threaded server
                       sender_socket
                           .lock()
                           .unwrap()
                           .send_to(&buffer[..bytes_read], dest_socket_addr)
                           .unwrap();


                       // Uncomment the following lines if you want to simulate delays between chunks
                       thread::sleep(Duration::from_secs(1));


                       println!(
                           "Sent chunk {}/{} to the threaded server.",
                           chunk_number + 1,
                           chunks
                       );
                   }


                   // Send the end marker to indicate the completion
                   sender_socket.lock().unwrap().send_to(END_MARKER, dest_socket_addr).unwrap();
                   println!("Sent the end marker to the client.");
                   let requests_ctr = *my_requests_clone3.lock().unwrap();
                   *my_requests_clone3.lock().unwrap() = requests_ctr - 1;
               }
               Err(_) => {}
           }
       }
   });


   loop {
       thread::sleep(Duration::from_secs(2));
   }
}