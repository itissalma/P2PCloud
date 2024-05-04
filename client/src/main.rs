// This is the client
extern crate steganography;
use steganography::util::*;
use steganography::encoder::Encoder;
use steganography::decoder::*;
use termion::color;
use std::str::FromStr;
// use std::error::Error;
// use std::{ net::UdpSocket, collections::HashMap };
// use show_image::{ ImageView, ImageInfo, create_window };
// use minifb::{ Key, Window, WindowOptions };
use std::{ net::UdpSocket, net::SocketAddr, collections::HashMap };
use std::net::{ IpAddr, Ipv4Addr };
use std::io::{ self, Read, Write };
use std::sync::mpsc;
// use std::io::Write;
use std::fs::File;
use std::time::Duration;
use std::thread;
use image::{ DynamicImage, GenericImageView, ImageBuffer, Rgba, io::Reader as ImageReader };
use show_image::{ ImageView, ImageInfo, create_window };
use minifb::{ Key, Scale, Window, WindowOptions };
// use image::{ DynamicImage, GenericImageView, ImageBuffer, ImageResult, Rgba };
// use std::path::Path;
use serde::{ Serialize, Deserialize };
use std::sync::{ Arc, Mutex };
use std::fs;
use bincode::deserialize;
extern crate base64;  
use chrono::Utc;
use std::io::*;

use steganography::encoder::*;
use steganography::decoder::*;
use steganography::util::*;


//ADD Rec ACK message to send the request.

const CHUNK_SIZE: usize = 65507; // Adjust the chunk size as needed
const BUFFER_SIZE: usize = 65507; // Buffer size for receiving data
const END_MARKER: &[u8] = b"END_OF_IMAGE";

#[derive(Serialize, Deserialize)]
struct ImageMessage {
    image_data: Vec<u8>,
}

struct User {
    username: String,
    logged_in: bool,
    // images_requested: Vec<DynamicImage>,
    // users_logged_in: HashMap<String, User>,
    dos: HashMap<String, SocketAddr>, // Key: Username, Value: IP address
    my_images: Vec<DynamicImage>,
    images_received: HashMap<String, Vec<HashMap<DynamicImage, i32>>>,
    images_sent: HashMap<String, Vec<HashMap<DynamicImage, i32>>>,
}

impl User {
    fn new() -> User {
        User {
            username: String::new(),
            logged_in: false,
            // images_requested: Vec::new(),
            // users_logged_in: HashMap::new(),
            dos: HashMap::new(),
            my_images: Vec::new(),
            images_received: HashMap::new(),
            images_sent: HashMap::new(),
        }
    }
    fn is_logged_in(&self) -> bool {
        self.logged_in
    }
}

fn encode_message_in_image(message: &str, source_image_path: &str, dest_image_path: &str) {
    // Convert the message to bytes
    //let payload = str_to_bytes(&message.to_string());

    let message_string = message.to_string(); // Create a variable to hold the string
    let payload = str_to_bytes(&message_string);

    // Load the source image
    let destination_image = file_as_dynamic_image(source_image_path.to_string());

    // Create an encoder
    let enc = Encoder::new(payload, destination_image);

    // Encode the message into the alpha channel of the image
    let result = enc.encode_alpha();

    // Save the new image with the hidden message
    save_image_buffer(result, dest_image_path.to_string());
}

fn decode_and_extract_message(filename: &str) -> String {
    // Load the image with the secret message
    let encoded_image = file_as_image_buffer(filename.to_string());

    // Create a decoder
    let dec = Decoder::new(encoded_image);

    // Decode the image by reading the alpha channel
    let out_buffer = dec.decode_alpha();

    // If there is no alpha, it's set to 255 by default so we filter those out
    let clean_buffer: Vec<u8> = out_buffer.into_iter()
                                          .filter(|b| *b != 0xff_u8)
                                          .collect();

    // Convert those bytes into a string we can read
    bytes_to_str(clean_buffer.as_slice()).to_string()
}


#[allow(non_snake_case)]
fn hasEncryptedCopy(image_name: &str) -> bool {
    let encrypted_directory = "my_images_encrypted";
    let encrypted_image_path = format!("{}/encrypted_{}", encrypted_directory, image_name);

    fs::metadata(&encrypted_image_path).is_ok()
}
#[allow(non_snake_case)]
fn hasLowCopy(image_name: &str) -> bool {
    let low_res_directory = "my_images_low_res";
    let low_res_image_path = format!("{}/low_res_{}", low_res_directory, image_name);

    fs::metadata(&low_res_image_path).is_ok()
}

fn send_image_and_receive_encrypted(
    socket: &UdpSocket,
    image_path: &str,
    server_address: &str
) -> DynamicImage {
    // Read the image data from a file
    println!("IMAGE PATH {}", image_path);
    let mut file = File::open(image_path).unwrap();
    let img_size = std::fs::metadata(image_path).unwrap().len();
    let chunks = ((img_size as f64) / (CHUNK_SIZE as f64)).ceil() as i32;

    // Create a message to indicate image encryption
    // let encrypt_message = format!("ENCRYPT,{}", image_path);
    // socket.send_to(encrypt_message.as_bytes(), server_address.to_owned() + ":8086").unwrap();
    // println!("Sent to {} for encryption", server_address.to_owned() + ":8086");

    // Loop to read and send chunks of data
    for chunk_number in 0..chunks {
        let mut buffer = [0u8; CHUNK_SIZE];
        let bytes_read = file.read(&mut buffer).unwrap();
        println!("Bytes Read: {:?}", bytes_read);

        // Send the current chunk of data to the server
        socket.send_to(&buffer[..bytes_read], server_address.to_owned() + ":8086").unwrap();
        //print the image chunk bytes
        // println!("Chunk data is {:?}", &buffer[..bytes_read]);
        println!(
            "Sent chunk {}/{} to the server {}.",
            chunk_number + 1,
            chunks,
            server_address.to_owned() + ":8086"
        );

        // Uncomment the following lines if you want to simulate delays between chunks
        thread::sleep(Duration::from_secs(1));
    }

    // Send the end marker to indicate the completion
    //socket.send_to(END_MARKER, server_address.to_owned() + ":8086").unwrap();

    println!("Number of Chunks: {}", chunks);
    println!("Image Size: {}", img_size);
    println!("Image {} sent to the server at {} for encryption.", image_path, server_address);

    // Receive the encrypted image data from the server
    let mut encrypted_image_data = Vec::new();
    let mut buffer = [0u8; BUFFER_SIZE];
    loop {
        let (bytes_read, _) = socket.recv_from(&mut buffer).unwrap();
        println!("HERE Bytes Read: {}", bytes_read);
        encrypted_image_data.extend_from_slice(&buffer[..bytes_read]);
        println!("recieved chunk data {:?} ", encrypted_image_data);
        if bytes_read < BUFFER_SIZE {
            println!("Bytes Read: {:?}", bytes_read);
            break;
        } else {
            println!("Bytes Read: {:?}", bytes_read);
        }
    }

    //save image recieved
    let image = image::load_from_memory(&encrypted_image_data).unwrap_or_else(|error| {
        eprintln!("Error: {:?}", error);
        // Handle the error or exit the program gracefully
        std::process::exit(1);
    });

    image
}

fn resize_cover_image(
    cover_image: DynamicImage,
    target_width: u32,
    target_height: u32
) -> DynamicImage {
    cover_image.resize_exact(target_width, target_height, image::imageops::FilterType::Triangle)
}


//add here ya allaa
//Takes cover image + embedded image (cover + embedded) and returns original image (embedded - cover)
// fn extract_first(image: DynamicImage, cover_image: DynamicImage) -> DynamicImage {
//     let image_width = image.width();
//     let image_height = image.height();

//     // Resize the cover image to match the dimensions of the received encrypted image
//     let resized_cover_image = resize_cover_image(cover_image, image_width, image_height);

//     let image_buffer = image.to_rgba8();
//     let mut extracted_buffer = ImageBuffer::<Rgba<u8>, _>::new(image_width, image_height);

//     for (x, y, pixel) in image_buffer.enumerate_pixels() {
//         // Bounds checking
//         if x < image_width && y < image_height {
//             let cover_pixel = resized_cover_image.get_pixel(x, y);
//             let (r, g, b, _) = (cover_pixel[0], cover_pixel[1], cover_pixel[2], cover_pixel[3]);
//             let (hr, hg, hb, ha) = (
//                 (pixel[0] & 0xf) << 4,
//                 (pixel[1] & 0xf) << 4,
//                 (pixel[2] & 0xf) << 4,
//                 255,
//             );
//             let rgba_pixel = Rgba([hr.wrapping_add(r), hg.wrapping_add(g), hb.wrapping_add(b), ha]);
//             extracted_buffer.put_pixel(x, y, rgba_pixel);
//         }
//     }

//     DynamicImage::ImageRgba8(extracted_buffer)
// }


fn extract_first(encoded_path: &str, output_path: &str) {
    // Load the encoded image
    let encoded_image = file_as_image_buffer(encoded_path.to_string());

    // Create a decoder and decode the message
    let decoder = Decoder::new(encoded_image);
    let out_buffer = decoder.decode_alpha();

    // Clean the buffer and convert it back to an image
    let clean_buffer: Vec<u8> = out_buffer.into_iter().filter(|b| *b != 0xff_u8).collect();
    let message = bytes_to_str(clean_buffer.as_slice());
    let decoded_image = base64::decode(message).expect("Failed to decode base64");

    // Save the decoded image
    let mut file = File::create(output_path).unwrap();
    file.write_all(&decoded_image).expect("Failed to write decoded image");
}

fn is_image_complete(chunks: &[Vec<u8>]) -> bool {
    // Check if the end marker exists at the end of the received data
    if let Some(last_chunk) = chunks.last() {
        last_chunk.ends_with(END_MARKER)
    } else {
        false
    }
}

fn view_image(image_path: String) {
    //Load the image file
    let img = image::open(image_path).unwrap();

    //Convert the image to RGBA format
    let rgba_image = img.to_rgba8();

    //Get image dimensions
    let (width, height) = rgba_image.dimensions();

    //Create a buffer to hold the data
    let mut buffer: Vec<u32> = Vec::with_capacity((width * height) as usize);

    for pixel in rgba_image.pixels() {
        let [r, g, b, a] = pixel.0;
        buffer.push(((r as u32) << 16) | ((g as u32) << 8) | (b as u32));
    }

    //Create a window
    let mut window = Window::new("Image", width as usize, height as usize, WindowOptions {
        resize: true,
        scale: Scale::X1,
        ..Default::default()
    }).unwrap_or_else(|e| {
        panic!("{}", e);
    });

    //Event loop for handling window updates
    while window.is_open() && !window.is_key_down(Key::Escape) {
        //Update the window with the new data in the buffer
        window.update_with_buffer(&buffer, width as usize, height as usize).unwrap();
    }
}

fn get_low_res_image(input_path: &str) -> DynamicImage {
    // Load the image from the file
    let image = image::open(input_path).unwrap();

    // Define the scale factor
    // let scale_factor = 0.15;

    // // Calculate the new dimensions based on the scale factor
    // let (width, height) = (
    //     ((image.width() as f32) * scale_factor) as u32,
    //     ((image.height() as f32) * scale_factor) as u32,
    // );

    // Resize the image to the new dimensions
    // let resized_image = image.resize_exact(width, height, image::imageops::FilterType::Gaussian);
    let resized_image = image.resize_exact(200, 200, image::imageops::FilterType::Gaussian);

    // Convert the DynamicImage to RgbaImage
    let rgba_image = resized_image.to_rgba8();

    // Return the low-resolution image
    DynamicImage::ImageRgba8(rgba_image)
}
#[allow(non_snake_case)]
fn send_after_login(
    input_directory: &str,
    encrypted_directory: &str,
    low_res_directory: &str,
    my_ip: &str,
    server_addresses: Vec<&str>
) {
    let socket = UdpSocket::bind(my_ip.to_owned() + ":9090").expect("Failed to bind to address");

    // Get a list of files in the input directory
    //let image_files = fs::read_dir(input_directory).unwrap();
    //  NOT NECESSARY --- TODO 8: error handling --> no images
    let image_files: Vec<String> = fs
        ::read_dir(input_directory)
        .unwrap()
        .filter_map(|entry| { entry.ok().and_then(|e| e.path().to_str().map(String::from)) })
        .collect();
    print!("\x1B[1m"); // ANSI escape code for bold text
    print!("\x1B[4m"); // ANSI escape code for underlined text
    print!("{}", color::Fg(color::LightRed));
    // println!("ABOUT TO SEND THE IMAGE THROUGH THE CHANNEL: {:?}", image_data);
    println!("Image Files: {:?}", image_files);
    print!("{}", color::Fg(color::Reset));
    print!("\x1B[0m"); // ANSI escape code to reset formatting
    io::stdout().flush().unwrap();

    for image_file in image_files {
        // Create the UDP socket and other necessary setup here
        for server in &server_addresses {
            println!("Server Addresssss: {}", server);
            socket.send_to(b"2", server.to_owned().to_owned() + ":8086").unwrap();
        }

        if
            let Some(file_name) = std::path::Path
                ::new(&image_file)
                .file_name()
                .and_then(|s| s.to_str())
        {
            let image_path = format!("{}/{}", input_directory, file_name);
            if !hasEncryptedCopy(&image_file) {
                for server in &server_addresses {
                    println!("Server Addresssss: {}", server);
                    socket.send_to(b"2", server.to_owned().to_owned() + ":8086").unwrap();
                }
                println!("Image being encrypted: {}", image_path);
                let mut buf = [0u8; BUFFER_SIZE];
                println!("Waiting for response...");

                if let Ok((n, responded_src)) = socket.recv_from(&mut buf) {
                    let ack_response = String::from_utf8_lossy(&buf[0..n]);
                    println!("Response is {} from {}", ack_response, responded_src);
                    if ack_response == "ACK" {
                        //Send to the responded_src the image to be encrypted
                        //let cover_image_path = "my_image.jpg";

                        //sleep for 1 second
                        thread::sleep(Duration::from_secs(1));
                        println!("Sent to Server {}", responded_src.ip());
                        let encrypted_image = send_image_and_receive_encrypted(
                            &socket,
                            &image_path,
                            &responded_src.ip().to_string()
                        );
                        let encrypted_image_path = format!(
                            "{}/encrypted_{}",
                            encrypted_directory,
                            file_name
                        );
                        encrypted_image.save(encrypted_image_path.clone()).unwrap();
                        println!("Encrypted image saved to: {}", encrypted_image_path);
                        //set timeout for one second
                        thread::sleep(Duration::from_secs(1));
                    }
                }
            }
            if !hasLowCopy(&image_file) {
                println!("Image being resized (low-resolution): {}", image_path);
                // Get the low-resolution image
                let low_res_image = get_low_res_image(&image_path);
                // Save the low-resolution image to the low-res directory
                let low_res_image_path = format!("{}/low_res_{}", low_res_directory, file_name);
                low_res_image.save(low_res_image_path.clone()).unwrap();
                println!("Low-resolution image saved to: {}", low_res_image_path);

                //set timeout for one second
                //thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

fn main() {
    let user_mutex = Arc::new(Mutex::new(User::new()));
    let user_mutex_clone = user_mutex.clone();
    let user_mutex_clone2 = user_mutex.clone();
    let my_ip = "127.0.0.7";
    let server_addresses = vec!["127.0.0.1", "127.0.0.2", "127.0.0.3"];
    let server_adresses_cloned = server_addresses.clone();
    let (logged_in_tx, logged_in_rx) = mpsc::channel();

    // // Deserialize the received bytes back into a tuple
    // let (bytes_received, _) = socket.recv_from(&mut buffer).expect("Failed to receive data");
    // let received_tuple: (String, i32) = deserialize(&buffer[..bytes_received]).expect("Failed to deserialize data");

    // Server Listener Loop

    thread::spawn(move || {
        let listener_serv_socket = UdpSocket::bind(my_ip.to_owned() + ":9091").expect(
            "Failed to bind to address"
        );
        let mut buf = [0u8; 65507];
        loop {
            match listener_serv_socket.recv_from(&mut buf) {
                Ok((bytes_received, src)) => {
                    println!("In server listener loop");
                    let received_message = String::from_utf8_lossy(
                        &buf[0..bytes_received]
                    ).to_string();
                    let parts: Vec<&str> = received_message.split(',').collect();
                    println!(
                        "Received {} bytes from {}: {}",
                        bytes_received,
                        src,
                        received_message
                    );
                    println!("Parts[0]: {}", parts[0]);
                    // TODO 11: Error HERE
                    if parts[0] == "ENTRY" {
                        println!("Parts[1]: {:?}, Parts[2]: {:?}", parts[1], parts[2]);
                        user_mutex_clone
                            .lock()
                            .unwrap()
                            .dos.insert(
                                parts[1].to_owned(),
                                SocketAddr::from_str(parts[2]).unwrap()
                            );
                    } else if parts[0] == "LOGIN_SUCCESS" {
                        let rest = &parts[1..];
                        // println!("MAP: {:?}", parts[1]);
                        println!("MAP: {:?}", rest);
                        for entry in rest {
                            let thing: Vec<&str> = entry.split(": ").collect();
                            let username: String = thing[0]
                                .chars()
                                .filter(|&c| c != '{' && c != '"' && c != '}' && c!= ' ')
                                .collect();
                            std::thread::sleep(std::time::Duration::new(1, 0));
                            let socket_addr: Vec<&str> = thing[1].split(":").collect();
                            let ip_addr: Vec<&str> = socket_addr[0].split(".").collect();
                            let port: String = socket_addr[1]
                                .chars()
                                .filter(|&c| c != '{' && c != '}')
                                .collect();
                            println!(
                                "{} - {} - {} - {}: {}",
                                ip_addr[0].parse::<u8>().unwrap(),
                                ip_addr[1].parse::<u8>().unwrap(),
                                ip_addr[2].parse::<u8>().unwrap(),
                                ip_addr[3].parse::<u8>().unwrap(),
                                port.parse::<u16>().unwrap()
                            );
                            let new_socket = SocketAddr::new(
                                IpAddr::V4(
                                    Ipv4Addr::new(
                                        ip_addr[0].parse::<u8>().unwrap(),
                                        ip_addr[1].parse::<u8>().unwrap(),
                                        ip_addr[2].parse::<u8>().unwrap(),
                                        ip_addr[3].parse::<u8>().unwrap()
                                    )
                                ),
                                port.parse::<u16>().unwrap()
                            );
                            user_mutex_clone.lock().unwrap().dos.insert(username, new_socket);
                        }

                        // sleep for 1 second
                        // println!("Username: {}", username);
                        // println!("Thing 2: {}", thing[1]);

                        // display the map
                        println!("Full List: {:?}", user_mutex_clone.lock().unwrap().dos);
                    } else {
                        println!("Invalid Message Received");
                    }
                }
                Err(e) => {
                    eprintln!("Error receiving data: {}", e);
                }
            }
        }
    });

    // Server Sender Loop (Only for Encryption and not Logging In)
    thread::spawn(move || {
        match logged_in_rx.recv() {
            Ok(is_logged_in) => {
                if is_logged_in {
                    let input_directory = "my_images";
                    let encrypted_directory = "my_images_encrypted";
                    let low_res_directory = "my_images_low_res";
                    // send_after_login(
                    //     input_directory,
                    //     encrypted_directory,
                    //     low_res_directory,
                    //     my_ip,
                    //     server_adresses_cloned
                    // );
                }
            }
            Err(_) => {}
        }
    });

    let mut imgs_rcv: Arc<Mutex<HashMap<String, u32>>> = Arc::new(Mutex::new(HashMap::new()));
    let mut imgs_rcv_clone = imgs_rcv.clone();
    let mut imgs_rcv_clone2 = imgs_rcv.clone();
    // Client Listener Thread
    thread::spawn(move || {
        // client_listener_loop(client_socket)

        // This thread should listen for two types of messages:
        // 1. Replies to previous requests made => Encrypted images or their new access rights. (OKAY)
        // 2. Incoming requests --> REQUEST_HIGH_RES, REQUEST_LOW_RES (OKAY)

        let client_listener_socket = UdpSocket::bind(my_ip.to_owned() + ":9001").expect(
            "Failed to bind to address"
        );
        let mut counter = 0;
        loop {
            let mut buf = [0u8; 1024];
            match client_listener_socket.recv_from(&mut buf) {
                Ok((n, src)) => {
                    let received_message = String::from_utf8_lossy(&buf[0..n]).to_string();
                    let rec_image_data = buf[..n].to_vec();
                    // println!("Received {} bytes from {}: {}", n, src, received_message);
                    let parts: Vec<&str> = received_message.split(',').collect();
                    // -------------------------------------------------------------------

                    //Parsing the received Message:

                    if parts.len() == 2 && parts[0] == "REQUEST_LOW_RES" {
                        // client_listener_socket

                        let image_files: Vec<String> = fs
                            ::read_dir("my_images_low_res")
                            .unwrap()
                            .filter_map(|entry|
                                entry.ok().and_then(|e| e.path().to_str().map(String::from))
                            )
                            .collect();

                        for image_file in image_files {
                            println!("Sending image: {}", image_file);
                            // Read the binary content of the image file
                            let image_bytes = fs::read(&image_file).unwrap();

                            let mut message_bytes = Vec::new();
                            message_bytes.extend_from_slice(b"SENDING_LOW_RES:");
                            message_bytes.extend_from_slice(&image_bytes);
                            println!("Message Bytes: {}", message_bytes.len());

                            // Send the image bytes over the UDP socket
                            if
                                let Err(err) = client_listener_socket.send_to(
                                    &image_bytes,
                                    src.ip().to_string().to_owned() + ":9091"
                                )
                            {
                                eprintln!("Error sending image '{}': {}", image_file, err);
                            } else {
                                println!("Image '{}' sent successfully", image_file);
                            }
                        }
                    } else if parts.len() == 3 && parts[0] == "REQUEST_HIGH_RES" {
                        // TODO 7:
                        // Fragment the chosen image in the directory and send it to the peer client.

                        let views = parts[2];
                        let image_files: Vec<String> = fs
                            ::read_dir("my_images_encrypted")
                            .unwrap()
                            .filter_map(|entry|
                                entry.ok().and_then(|e| e.path().to_str().map(String::from))
                            )
                            .collect();
                            println!("Image FIles: {:?}", image_files);
                        let img_idx = parts[1];
                        let image_path = &image_files[img_idx.parse::<usize>().unwrap()];
                        println!("The Client is asking to view the image for {} times", views);
                        // ask the user if he agrees or not
                        println!("Do you agree to send the image? (y/n)");
                        let mut input = String::new();
                        io::stdout().flush().unwrap();
                        io::stdin().read_line(&mut input).expect("Failed to read input");
                        let input = input.trim().to_string();
                        // if no, ask the user to input the number of views granted

                        if input == "n" {
                            println!("Enter the number of views you wish to grant: ");
                            let mut raw_views = String::new();
                            io::stdin().read_line(&mut raw_views).expect("Failed to read input");
                            let views = raw_views.trim().to_string();
                            if views != "0" {
                                
                                // Use steganography function --> embed view number, image name. then send the image
                                let message = format!("{},{}", image_path, raw_views);
                                let hidden_image = format!("hidden_{}", image_path);
                                encode_message_in_image(&message, &image_path, &hidden_image);
                                
                                let image_bytes = fs::read(&hidden_image).unwrap();
                                if
                                    let Err(err) = client_listener_socket.send_to(
                                        &image_bytes,
                                        src.ip().to_string().to_owned() + ":9091"
                                    )
                                {
                                    eprintln!("Error sending image '{}': {}", image_path, err);
                                } else {
                                    println!("Image '{}' sent successfully", image_path);
                                }
                            }
                        } else if input == "y" {
                            // send the image
                            // Use steganography function --> embed view number, image name. then send the image
                            let message = format!("{},{}", image_path, views);
                            let hidden_image = format!("hidden_{}", image_path);
                            println!("Message: {}, hidden_image: {}, image_path {}", message, hidden_image, image_path);
                            encode_message_in_image(&message, image_path, &hidden_image);

                            let image_bytes = fs::read(&image_path).unwrap();
                            if
                                let Err(err) = client_listener_socket.send_to(
                                    &image_bytes,
                                    src.ip().to_string().to_owned() + ":9091"
                                )
                            {
                                eprintln!("Error sending image '{}': {}", image_path, err);
                            } else {
                                println!("Image '{}' sent successfully", image_path);
                            }
                        } else {
                            println!("Invalid input");
                        }
                    } else if parts.len() == 3 && parts[0] == "EDIT_ACCESS_RIGHTS" {
                        // Need to alter the imgs_received HM in the user struct
                        // Update map entry
                        // let entry = imgs_rcv.lock().unwrap().entry(parts[1].clone().to_string()).or_insert(0);
                        // *entry = parts[2].parse::<u32>().unwrap();
                        //imgs_rcv.lock().unwrap()[parts[1]] = &parts[2].parse::<u32>().unwrap();
                        
                        let key = parts[1].clone().to_string();
                        let mut imgs_rcv_lock = imgs_rcv_clone.lock().unwrap();
                        let entry = imgs_rcv_lock.entry(key).or_insert(0);
                        *entry = parts[2].parse::<u32>().unwrap();

                    } else if received_message.starts_with("SENDING_LOW_RES:") {
                        counter += 1;
                        //print the received message
                        //println!("Received message: {:?}", received_message);
                        // println!("REceived Image data {:?}", rec_image_data);
                        // println!("Received SENDING_LOW_RES {}", received_message.len());
                        let image_bytes = &rec_image_data["SENDING_LOW_RES:".len()..];

                        println!("Received image bytes: {}", image_bytes.len());
                        // Save all images received into "low_res_images_folder" which already exists
                        
                        thread::sleep(Duration::from_secs(1));
                        let timestamp = Utc::now().timestamp();
                        let image_file_path =
                            format!("low_res_images/low_res_image{}_{}.jpg", timestamp, counter);
                        let mut image_file = File::create(&image_file_path).expect(
                            "Failed to create image file"
                        );
                        //image_file.write_all(image_bytes).expect("Failed to write image file");
                        //println!("Image received successfully and saved to {}", image_file_path);
                        if let Err(err) = std::fs::write(&image_file_path, image_bytes) {
                            eprintln!("Error saving image: {}", err);
                        } else {
                            println!("Image saved successfully to {}", image_file_path);
                        }
                        
                    } else {
                        // It is an encrypted image 
                        // Add entry to map.
                        // TODO 111: need to receive from the client the 
                        let output = decode_and_extract_message(&received_message);
                        let output_parts: Vec<&str> = output.split(',').collect();

                        let key = output_parts[1].clone().to_string();
                        let mut imgs_rcv_lock = imgs_rcv_clone.lock().unwrap();
                        let entry = imgs_rcv_lock.entry(key).or_insert(0);
                        *entry = output_parts[2].parse::<u32>().unwrap();

                        

                        //imgs_rcv.insert(output_parts[0], output_parts[1].parse().unwrap());

                    }
                }
                Err(e) => {
                    eprintln!("Error receiving data: {}", e);
                }
            }
        }
    });

    // Client Worker & Sender Thread (Main Focus)
    thread::spawn(move || {
        let client_sender_socket = UdpSocket::bind(my_ip.to_owned() + ":9000").expect(
            "Failed to bind to address"
        );
        println!("I am here");
        loop {
            if !user_mutex_clone2.lock().unwrap().logged_in {
                println!("1. Login");
                print!("Select an option: ");
            } else {
                println!("1. List Online Users");
                println!("2. Request Low-Resolution Images");
                println!("3. Request High-Resolution Images");
                println!("4. Edit Access Rights");
                println!("5. View Received Encrypted Images");
                println!("6. Logout");
                print!("Select an option: ");
            }

            let mut input = String::new();
            io::stdin().read_line(&mut input).expect("Failed to read input");
            let input = input.trim().to_string();
            let login_socket = UdpSocket::bind(my_ip.to_owned() + ":9999").expect(
                "Failed to bind to address"
            );

            match input.as_str() {
                "1" => {
                    if !user_mutex_clone2.lock().unwrap().logged_in {
                        // Handle login
                        println!("Enter username: ");
                        let mut new_username = String::new();
                        io::stdin().read_line(&mut new_username).expect("Failed to read input");
                        let new_username_trimmed = new_username.trim().to_string();

                        for server in &server_addresses {
                            let login_message = format!("LOGIN,{}", new_username_trimmed);
                            login_socket
                                .send_to(
                                    login_message.as_bytes(),
                                    server.to_owned().to_owned() + ":8086"
                                )
                                .expect("Failed to send login request");
                        }

                        let mut buf = [0u8; 1024];
                        if let Ok((n, responded_src)) = login_socket.recv_from(&mut buf) {
                            println!("Response received from server {}", responded_src);
                            let login_response = String::from_utf8_lossy(&buf[0..n]);
                            let parts: Vec<&str> = login_response.split(',').collect();
                            println!("Parts: {:?}", parts);
                            if parts[0] == "ACK" {
                                user_mutex_clone2.lock().unwrap().logged_in = true;
                                user_mutex_clone2.lock().unwrap().username = new_username_trimmed;
                                println!("Login successful.");
                                logged_in_tx.send(true).unwrap();
                            } else {
                                println!("Login failed. Please try again.");
                            }
                        }
                    } else {
                        // Handle list online users
                        println!("Online users:");
                        for username in user_mutex_clone2.lock().unwrap().dos.keys() {
                            println!("{}", username);
                        }
                    }
                }
                "2" => {
                    //request low res images
                    println!("Enter username: ");
                    let mut other_username = String::new();
                    io::stdin().read_line(&mut other_username).expect("Failed to read input");
                    let other_username_trimmed = other_username.trim().to_string();

                    let request_message = format!("REQUEST_LOW_RES,{}", other_username_trimmed);
                    let client_ip = &user_mutex_clone2.lock().unwrap().dos[&other_username_trimmed];
                    client_sender_socket
                        .send_to(
                            request_message.as_bytes(),
                            client_ip.ip().to_string().to_owned() + ":9001"
                        )
                        .expect("Failed to send low resolution request");
                }
                "3" => {
                    //request high res images
                    println!("Enter username: ");
                    let mut other_username = String::new();
                    io::stdin().read_line(&mut other_username).expect("Failed to read input");
                    let other_username_trimmed = other_username.trim().to_string();

                    println!("Enter image index: ");
                    let mut raw_img_idx = String::new();
                    io::stdin().read_line(&mut raw_img_idx).expect("Failed to read input");
                    let img_idx = raw_img_idx.trim().to_string();

                    // Ask the user for how many views you wish to be granted
                    println!("Enter number of views: ");
                    let mut raw_views = String::new();
                    io::stdin().read_line(&mut raw_views).expect("Failed to read input");
                    let views = raw_views.trim().to_string();
                    let request_message = format!("REQUEST_HIGH_RES,{},{}", img_idx, views);
                    let client_ip = &user_mutex_clone2.lock().unwrap().dos[&other_username_trimmed];
                    client_sender_socket
                        .send_to(
                            request_message.as_bytes(),
                            client_ip.ip().to_string().to_owned() + ":9001"
                        )
                        .expect("Failed to send high resolution request");
                }
                "4" => {
                    //edit access rights
                    println!("Enter username: ");
                    let mut other_username = String::new();
                    io::stdin().read_line(&mut other_username).expect("Failed to read input");
                    let other_username_trimmed = other_username.trim().to_string();

                    println!("Enter image name: ");
                    let mut raw_img_idx = String::new();
                    io::stdin().read_line(&mut raw_img_idx).expect("Failed to read input");
                    let img_idx = raw_img_idx.trim().to_string();

                    println!("Enter new access rights: ");
                    let mut new_access_rights = String::new();
                    io::stdin().read_line(&mut new_access_rights).expect("Failed to read input");
                    let new_access_rights_trimmed = new_access_rights.trim().to_string();

                    let request_message = format!(
                        "EDIT_ACCESS_RIGHTS,{},{}",
                        img_idx,
                        new_access_rights_trimmed
                    );
                    let client_ip = &user_mutex_clone2.lock().unwrap().dos[&other_username_trimmed];
                    client_sender_socket
                        .send_to(
                            request_message.as_bytes(),
                            client_ip.ip().to_string().to_owned() + ":9001"
                        )
                        .expect("Failed to send edit access rights request");
                }
                "5" => {
                    // ask user for the name of the image
                    println!("Enter image name: ");
                    let mut image_name = String::new();
                    io::stdin().read_line(&mut image_name).expect("Failed to read input");
                    let image_name_trimmed = image_name.trim().to_string();
                    // Open the directory "received_encrypted_images" and read the image :that corresponds to the user input
                    // let received_image_path = format!("encrypted_{}.jpeg", image_name_trimmed);

                    println!("Received Image Path {}", image_name_trimmed);
                    // let image = image::open(received_image_path).unwrap();
                    // TODO 12: Decrypt and view image

                    view_image(image_name_trimmed);
                }
                "6" => {
                    //logout
                    for server in &server_addresses {
                        let login_message = format!(
                            "LOGOUT,{}",
                            user_mutex_clone2.lock().unwrap().username
                        );
                        login_socket
                            .send_to(
                                login_message.as_bytes(),
                                server.to_owned().to_owned() + ":8086"
                            )
                            .expect("Failed to send logout request");
                    }
                    user_mutex_clone2.lock().unwrap().dos.clear();
                }
                _ => {
                    println!("Invalid option. Please try again.");
                }
            }
        }
    });

    loop {
        // sleep for 2 seconds
        thread::sleep(Duration::from_secs(2));
    }
}
