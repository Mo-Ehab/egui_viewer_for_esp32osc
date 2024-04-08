#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

//a gui for the esp32 oscilliscope project, communicates through UART and views signal using egui


use eframe::egui;
use egui_plot::{Bar, BarChart, Legend, Line, Plot, PlotBounds, PlotPoints};
mod frame_history;
use std::{str, thread};
use std::sync::mpsc::{channel, Receiver, Sender};

use std::time::Duration;
use std::io::Read;
use realfft::RealFftPlanner;

#[derive(PartialEq)]
enum Freq {On, Off}


#[derive(Clone, Copy)]
struct Viewerdata{
    xscale: f64,
    yscale: f64,
    frozen: bool,
}


fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([720.0, 480.0]),
        ..Default::default()
    };


    let mut uart_data = UARTdata::default(); // Data for the UART reader thread
    let (uart_send , uart_receive ) = channel(); // Channel to send voltage data from UART thread to main thread
    let (main_send , main_receive ) = channel(); // Channel to send viewer data from main thread to receiver

    // UART receive thread
    thread::spawn(move || {
        loop{
            uart_data.viewerdata = main_receive.try_iter().last().unwrap_or( uart_data.viewerdata);
            uart_data.uart_thread();
            uart_send.send(uart_data.graph.clone()).expect("Channel died")
        }

    });

    // Main (viewing) thread
    eframe::run_native(
        "Osci",
        options,
        Box::new(|_cc| {
            Box::<MyApp>::new(MyApp {
                graph: vec![[0.0, 0.0]],
                bounded: true,
                frame_history : Default::default(),
                viewerdata: Viewerdata { xscale: 10.0, yscale: 10.0, frozen:true },
                uart_receiver: uart_receive,
                main_send: main_send,
                freq : Freq::Off
            })
        }),
    )
}


// Egui viewing data
struct MyApp {
    bounded: bool,
    graph: Vec<[f64; 2]>,
    frame_history: crate::frame_history::FrameHistory,
    viewerdata: Viewerdata,
    uart_receiver: Receiver<Vec<[f64; 2]>>,
    main_send: Sender<Viewerdata>,
    freq: Freq
}

// UART thread Data
struct UARTdata{
    viewerdata: Viewerdata,
    graph: Vec<[f64; 2]>
}

impl Default for UARTdata {
    fn default() -> Self {
        Self {
            graph: vec![[0.0, 0.0]],
            viewerdata: Viewerdata { xscale: 10.0, yscale: 10.0, frozen:true },
        }
    }
}

pub trait UARTthread {
    fn uart_thread(&mut self) ;
}

impl UARTthread for UARTdata {

    fn uart_thread(&mut self) {

        let mut port = serialport::new("COM12", 921600 )
            .timeout(Duration::from_millis(100))
            .open().expect("Failed to open port"); // Connect to UART port

        let mut serial_buf   =[0; 512];
        port.read_exact(&mut serial_buf).expect("Found no UART_data!"); // Read from UART to buffer

        let data_string = String::from_utf8_lossy(&serial_buf) ; //convert buffer data to one long string
        let data_vector : Vec<&str> = data_string.split(", ").collect(); // Convert string data to array of strings
        let number_data : Vec<f64>  = data_vector.iter().map(|&f| f.parse::<f64>().unwrap_or(0.0)).collect(); // Convert data to vector of float values

        // Update graph if UI not frozen 
        if !self.viewerdata.frozen{

            for (i, item) in number_data.iter().enumerate(){
                if i ==0 || i ==1 || i > (number_data.len() - 2){   // truncate readings at start and end of port reading because they give wrong readings
                }
                else{
                    if self.graph.last().unwrap()[0] + (500.0/1000000.0) > self.viewerdata.xscale{ // Wrap data back to the start of graph
                        self.graph.push([self.graph.last().unwrap()[0] + (500.0/1000000.0) , -1.0]);
                        self.graph.push([0.0 + (500.0/1000000.0) , -1.0]);
                    } 
                    else{
                        self.graph.push([self.graph.last().unwrap()[0] + (500.0/1000000.0) , item/1241.1]); // Append readings to graph, currently one reading every 500 microsecond
                    }
                }

                if self.viewerdata.xscale * (1000000.0/500.0) < self.graph.len() as f64{ // Remove old readings to save memory and compute
                    self.graph.drain(..( self.graph.len() as f64 - self.viewerdata.xscale * (1000000.0/500.0)  )  as usize );
                }
            }

        }


    }

}

// Main thread to view data
impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
    
        ctx.request_repaint(); // Update the screen as fast as possible to view new readings

        let my_plot = Plot::new("My Plot").legend(Legend::default());
        let mut plot_rect = None;

        self.graph = self.uart_receiver.try_iter().last().unwrap_or(self.graph.clone()); // Receive data from uart thread
        self.main_send.send(self.viewerdata.clone()).ok(); // Send data to uart thread

        self.frame_history.on_new_frame(ctx.input(|i| i.time), frame.info().cpu_usage); // Calculate frame processing time

        egui::TopBottomPanel::bottom("bottom_panel") // show fram processing time
            .min_height(15.0)
            .show(ctx, |ui| {
                self.frame_history.ui(ui);
            });

        egui::CentralPanel::default().show(ctx, |ui| {

            ui.vertical_centered(|ui| {
                ui.heading("Eosci-scope");
            });

            ui.horizontal(|ui| {

                ui.vertical_centered(|ui| {

                    ui.horizontal_centered(|ui| {
                        ui.selectable_value(&mut self.freq, Freq::Off, "Time");
                        ui.selectable_value(&mut self.freq, Freq::On, "Frequency");
                    });

                    ui.horizontal(|ui| {
                        ui.toggle_value(&mut self.bounded, "Bounded"); // Bound graph to current values
                    });
                    
                    ui.horizontal(|ui| {
                        ui.toggle_value(&mut self.viewerdata.frozen, "Frozen"); // Freeze new data
                    });

                });

                ui.vertical_centered(|ui| {
                    ui.horizontal(|ui| {
                        ui.label("X-Axis: ");
        
                        if ui.add(egui::Button::new("  -  ")).clicked() { 
                            self.viewerdata.xscale *= 2.0; // Zoom out X-axis
                        }
                        if ui.add(egui::Button::new("  +  ")).clicked() {
                            self.viewerdata.xscale /= 2.0; // Zoom in X-axis
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Y-Axis: ");
        
                        if ui.add(egui::Button::new("  -  ")).clicked() {
                            self.viewerdata.yscale *= 2.0; // Zoom out Y-axis
                        }
                        if ui.add(egui::Button::new("  +  ")).clicked() {
                            self.viewerdata.yscale /= 2.0; // Zoom in Y-axis
                        }
                    });
                });
            });



            let inner = my_plot.show(ui, |plot_ui| {
                if self.bounded{
                    plot_ui.set_plot_bounds(PlotBounds::from_min_max([ 0.0 ,0.0], [ self.viewerdata.xscale ,self.viewerdata.yscale]))
                }

                if self.freq == Freq::On{ // Basic try for fft based spectrum analyzer
                    let mut real_planner = RealFftPlanner::<f64>::new();
                    let r2c = real_planner.plan_fft_forward(self.graph.len());

                    let mut buffer: Vec<f64>  = self.graph.iter().map(|&x| x[1] ).collect() ;
                    let mut spectrum = r2c.make_output_vec();

                    r2c.process(&mut buffer, &mut spectrum).unwrap();

                    let c: Vec<Bar> = spectrum.iter().enumerate().map(|(i, &x,)| Bar::new(  i as f64 /1000.0 , x.norm()/ f64::sqrt( buffer.len() as f64) ).width(0.0005) ).collect();
                    plot_ui.bar_chart(BarChart::new(c ).name("CH1"));
                }
                else{
                    plot_ui.line(Line::new(PlotPoints::from(self.graph.clone())).name("CH1")); // View data
                }

            });

            plot_rect = Some(inner.response.rect);


        });


    }
}