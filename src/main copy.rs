#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;
use egui_plot::{PlotBounds , Legend, Line, Plot, PlotPoints};
mod frame_history;
use std::net::ToSocketAddrs;
use std::{str, thread};
use std::sync::mpsc::{channel, Receiver, Sender};

use std::time::Duration;
use std::{io::{ Read, Write}, net::TcpStream};

#[derive(Clone, Copy)]
struct Sizes{
    xscale: f64,
    yscale: f64,
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
                addr: "192.168.8.128:8888".to_owned(),
                graph: vec![[0.0, 0.0]],
                bounded: true,
                frame_history : Default::default(),
                sizes: Sizes { xscale: 10.0, yscale: 10.0 },
                recvr: recv,
                sendback: sendback
            })
        }),
    )
}



struct MyApp {
    addr: String,
    bounded: bool,
    graph: Vec<[f64; 2]>,
    frame_history: crate::frame_history::FrameHistory,
    sizes: Sizes,
    recvr: Receiver<Vec<[f64; 2]>>,
    sendback: Sender<Sizes>

}



struct ImportantData{
    connected : bool,
    stream : Option<TcpStream>,
    sizes: Sizes,
    graph: Vec<[f64; 2]>

}

impl Default for ImportantData {
    fn default() -> Self {
        Self {
            stream : TcpStream::connect_timeout(&"192.168.8.128:8888".to_socket_addrs().unwrap().next().expect("error in address"), Duration::from_secs(1)).ok(),
            graph: vec![[0.0, 0.0]],
            connected: false,
            sizes: Sizes { xscale: 10.0, yscale: 10.0 },
        }
    }
}

pub trait RecvData {
    fn recv_data(&mut self) ;
}
impl RecvData for ImportantData {

    fn recv_data(&mut self) {

        if  !self.connected{
            self.stream = TcpStream::connect_timeout(&"192.168.8.128:8888".to_socket_addrs().unwrap().next().expect("error in address"), Duration::from_millis(1000)).ok();
            if !self.stream.is_none(){
                self.connected = true
            }

        }
        else{

            let mut buff:Vec<u8> = Vec::with_capacity(5);
            let mut timebuff:Vec<u8> = Vec::with_capacity(10);

            let mut s: &TcpStream  = self.stream.as_ref().expect("not connected");
            s.set_read_timeout(Some(Duration::from_millis(500)));
            
            s.write("C".as_bytes());

            let res = s.take(5).read_to_end(&mut buff);
            if res.is_err(){
                self.connected = false
            }
            s.take(10).read_to_end(&mut timebuff);
            
            let r = str::from_utf8(&buff).unwrap_or("failed").to_owned() ;
            
            if r.is_empty() {
                self.connected = false;
            }


            let fixed = r.parse().unwrap_or(0.0);

            let t = str::from_utf8(& timebuff).unwrap_or("failed").to_owned() ;

            let tfixed = t.parse().unwrap_or(0.0);

            if self.graph.last().unwrap()[0] + (tfixed/1000000.0) > self.sizes.xscale{
                self.graph = vec![[0.0 , fixed/19859.09]];
            }
            else{
                self.graph.push([self.graph.last().unwrap()[0] + (tfixed/1000000.0) , fixed/19859.09]);

            }
            

        }

    }
    

}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
    


        let my_plot = Plot::new("My Plot").legend(Legend::default());
        let mut plot_rect = None;

        self.graph = self.recvr.try_iter().last().unwrap_or(self.graph.clone());
        self.sendback.send(self.sizes.clone());

        self.frame_history.on_new_frame(ctx.input(|i| i.time), frame.info().cpu_usage);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Signal");
            self.frame_history.ui(ui);

            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.addr));
                if ui.add(egui::Button::new("Connect")).clicked() {

                    
                }
            });

            ui.horizontal(|ui| {
                ui.label("X-Axis: ");

                if ui.add(egui::Button::new("  -  ")).clicked() {
                    self.sizes.xscale /= 2.0;
                }
                if ui.add(egui::Button::new("  +  ")).clicked() {
                    self.sizes.xscale *= 2.0;

                }
            });

            ui.horizontal(|ui| {
                ui.label("Y-Axis: ");

                if ui.add(egui::Button::new("  -  ")).clicked() {
                    self.sizes.yscale /= 2.0;
                }
                if ui.add(egui::Button::new("  +  ")).clicked() {
                    self.sizes.yscale *= 2.0;
                }
            });
            
            ctx.request_repaint();
            ui.toggle_value(&mut self.bounded, "Bounded");

            let inner = my_plot.show(ui, |plot_ui| {
                if self.bounded{
                    plot_ui.set_plot_bounds(PlotBounds::from_min_max([ 0.0 ,0.0], [ self.sizes.xscale ,self.sizes.yscale]))
                }
                plot_ui.line(Line::new(PlotPoints::from(self.graph.clone())).name("CH1"));

            });

            plot_rect = Some(inner.response.rect);

        });
    }
}