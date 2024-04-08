#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;
use egui_plot::{Bar, BarChart, Legend, Line, Plot, PlotBounds, PlotPoints};
mod frame_history;
use std::{str, thread};
use std::sync::mpsc::{channel, Receiver, Sender};

use std::time::Duration;
use std::io::Read;
use realfft::RealFftPlanner;

#[derive(PartialEq)]
enum Freq { On, Off  }


#[derive(Clone, Copy)]
struct Sizes{
    xscale: f64,
    yscale: f64,
    connected: bool,
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([720.0, 480.0]),
        ..Default::default()
    };

    let mut data = ImportantData::default();
    let (send , recv ) = channel();
    let (sendback , recvback ) = channel();


    thread::spawn(move || {

        loop{
            data.sizes = recvback.try_iter().last().unwrap_or( data.sizes);
            data.recv_data();
            send.send(data.graph.clone()).unwrap()
        }

    });

    eframe::run_native(
        "Osci",
        options,
        Box::new(|_cc| {
            Box::<MyApp>::new(MyApp {
                graph: vec![[0.0, 0.0]],
                bounded: true,
                frame_history : Default::default(),
                sizes: Sizes { xscale: 10.0, yscale: 10.0, connected:true },
                recvr: recv,
                sendback: sendback,
                freq : Freq::Off
            })
        }),
    )
}



struct MyApp {
    bounded: bool,
    graph: Vec<[f64; 2]>,
    frame_history: crate::frame_history::FrameHistory,
    sizes: Sizes,
    recvr: Receiver<Vec<[f64; 2]>>,
    sendback: Sender<Sizes>,
    freq: Freq

}



struct ImportantData{
    sizes: Sizes,
    graph: Vec<[f64; 2]>

}

impl Default for ImportantData {
    fn default() -> Self {
        Self {
            graph: vec![[0.0, 0.0]],
            sizes: Sizes { xscale: 10.0, yscale: 10.0, connected:true },
        }
    }
}

pub trait RecvData {
    fn recv_data(&mut self) ;
}
impl RecvData for ImportantData {

    fn recv_data(&mut self) {

        let mut port = serialport::new("COM12", 921600 )
            .timeout(Duration::from_millis(100))
            .open().expect("Failed to open port");


        let mut serial_buf   =[0; 512];


        port.read_exact(&mut serial_buf).expect("Found no data!");


        let k = String::from_utf8_lossy(&serial_buf) ;

        let kt :Vec<&str> = k.split(", ").collect();

        let ktt : Vec<f64>  = kt.iter().map(|&f| f.parse::<f64>().unwrap_or(0.0)).collect();
        if self.sizes.connected{

            for (i, item) in ktt.iter().clone().enumerate(){
                if i ==0 || i ==1 || i > (ktt.len() - 2){                
                }
                else{
                    if self.graph.last().unwrap()[0] + (500.0/1000000.0) > self.sizes.xscale{
                        self.graph.push([self.graph.last().unwrap()[0] + (500.0/1000000.0) , -1.0]);
                        self.graph.push([0.0 + (500.0/1000000.0) , -1.0]);
                    }
                    else{
                        self.graph.push([self.graph.last().unwrap()[0] + (500.0/1000000.0) , item/1241.1]);
                    }
                }
                if self.sizes.xscale * (1000000.0/500.0) < self.graph.len() as f64{
                    self.graph.drain(..( self.graph.len() as f64 - self.sizes.xscale * (1000000.0/500.0)  )  as usize );
                }
            }

        }


    }

}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
    
        ctx.request_repaint();

        let my_plot = Plot::new("My Plot").legend(Legend::default());
        let mut plot_rect = None;

        self.graph = self.recvr.try_iter().last().unwrap_or(self.graph.clone());
        self.sendback.send(self.sizes.clone()).ok();

        self.frame_history.on_new_frame(ctx.input(|i| i.time), frame.info().cpu_usage);

        egui::TopBottomPanel::bottom("bottom_panel")
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
                            ui.toggle_value(&mut self.bounded, "Bounded");
                    });
                    

                    ui.horizontal(|ui| {
                            ui.toggle_value(&mut self.sizes.connected, "Frozen");
                    });

                });

                ui.vertical_centered(|ui| {
                    ui.horizontal(|ui| {
                        ui.label("X-Axis: ");
        
                        if ui.add(egui::Button::new("  -  ")).clicked() {
                            self.sizes.xscale *= 2.0;
                        }
                        if ui.add(egui::Button::new("  +  ")).clicked() {
                            self.sizes.xscale /= 2.0;
        
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Y-Axis: ");
        
                        if ui.add(egui::Button::new("  -  ")).clicked() {
                            self.sizes.yscale *= 2.0;
                        }
                        if ui.add(egui::Button::new("  +  ")).clicked() {
                            self.sizes.yscale /= 2.0;
                        }
                    });
                });
            });



            let inner = my_plot.show(ui, |plot_ui| {
                if self.bounded{
                    plot_ui.set_plot_bounds(PlotBounds::from_min_max([ 0.0 ,0.0], [ self.sizes.xscale ,self.sizes.yscale]))
                }

                if self.freq == Freq::On{
                    let mut real_planner = RealFftPlanner::<f64>::new();
                    let r2c = real_planner.plan_fft_forward(self.graph.len());


                    let mut buffer: Vec<f64>  = self.graph.iter().map(|&x| x[1] ).collect() ;
                    let mut spectrum = r2c.make_output_vec();


                    r2c.process(&mut buffer, &mut spectrum).unwrap();

                    let c: Vec<Bar> = spectrum.iter().enumerate().map(|(i, &x,)| Bar::new(  i as f64 /1000.0 , x.norm()/ f64::sqrt( buffer.len() as f64) ).width(0.0005) ).collect();
                    plot_ui.bar_chart(BarChart::new(c ).name("CH1"));
                }
                else{
                    plot_ui.line(Line::new(PlotPoints::from(self.graph.clone())).name("CH1"));


                }

            });

            plot_rect = Some(inner.response.rect);


        });



    }
}