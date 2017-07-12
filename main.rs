use std::io::*;
use std::fs::File;
use std::str;
use std::path::Path;
use std::process::{exit, Command, Stdio, Child};
use std::env;
use std::ffi::CString;
use std::os::unix::io::{FromRawFd,AsRawFd,IntoRawFd};
extern crate libc;
                   
//TODO : Add parser to deal with "|" that is an ordinary string. e.g. echo "abc|cba"

//Feature : "cd -" => last directory "cd ~" = "cd" => /home
//Feature : Error handling for build-in commands

fn cd(args: Vec<&str>, oldpwd: &str) -> () {
	let target;
    if args.len() > 1{
    	target = args[1].trim();
    }
    else{
    	target = "~";
    }
    if target == "~"{
    	unsafe{libc::chdir(CString::new(env::home_dir().unwrap().to_str().unwrap()).unwrap().as_ptr())};
    }
    else if target == "-"{
    	println!("{}",oldpwd);
    	unsafe{libc::chdir(CString::new(oldpwd).unwrap().as_ptr())};
    }
    else{
    	if unsafe{libc::chdir(CString::new(target).unwrap().as_ptr())} == -1{
    		println!("Error: No such file or directory");
    	}	
    }
}

fn kill(arg: &str) -> () {
	unsafe{
		let res = libc::waitpid(i32::from_str_radix(arg,10).unwrap(), std::ptr::null_mut(), libc::WNOHANG);
		if res == 0{
			libc::kill(i32::from_str_radix(arg,10).unwrap(),15);
		}
		else{
			println!("Error: No such process");
		}
	}
}

fn jobs(history: Vec<String>, joblist: &mut Vec<Vec<Child>>) -> () {
	for cmdid in 0..joblist.len(){
		for child in &joblist[cmdid]{
			let res = unsafe{libc::waitpid(child.id() as i32, std::ptr::null_mut(), libc::WNOHANG)};
			if res==0 {
				let mut name = history[cmdid].clone().replace("&","");
				while match name.find("  ") {Some(_) => true, None => false}
        		{
        			name = name.replace("  "," ");
        		}
				println!("{}",name.trim());
			}
		}
	}
}

fn parse(cmd: &str) -> (&str, Vec<&str>){
	let args: Vec<_> = cmd.split_whitespace().collect();
	if args.len() >= 1{
		(args[0], args)
	}
	else{
		("",vec![""])
	}
}

fn checkerr(cmds: Vec<&str>) -> bool{
	for id in 0..cmds.len(){
		if id!= cmds.len()-1 && match cmds[id].find("&") { Some(_) => true, None => false}{
			println!("Error: & can appear only after the last command");
			return false;
		}
		if id!= cmds.len()-1 && match cmds[id].find(">") { Some(_) => true, None => false}{
			println!("Error: > can appear only after the last command");
			return false;
		}
		if id!= 0 && match cmds[id].find("<") { Some(_) => true, None => false}{
			println!("Error: < can appear only in the first command");
			return false;
		}
	}
	return true;
}

fn execute(mut oldpwd:&mut String, joblist:&mut Vec<Vec<Child>>, history: &Vec<String>, cmds: &str) -> () {
	if !match cmds.find("&") { Some(_) => true, None => false}{
		joblist.push(vec![]);
	}
	let mut commands: Vec<_> = cmds.split(" | ").collect();
	if !checkerr(commands.clone()){
		return;
	}
	let redirectin = match commands[0].find("<") { Some(_) => true, None => false};
	let redirectout = match commands[commands.len()-1].find(">") { Some(_) => true, None => false};
	let background = match commands[commands.len()-1].find("&") { Some(_) => true, None => false};
	let mut outfile = "";
	let mut infile = "";
	let mut res = "".to_string();
	let mut processes:Vec<Child> = vec![];
	let mut buildin = false;
	if background{
		let temp: Vec<_> = commands[commands.len()-1].split("&").collect();
		let run = temp[0].trim();
		let length = commands.len()-1;
		commands[length] = run;
	}
	if redirectout{
		let temp: Vec<_> = commands[commands.len()-1].split(">").collect();
		let run = temp[0].trim();
		let length = commands.len()-1;
		commands[length] = run;
		outfile = temp[1].trim();
		if outfile == ""{
			println!("Error: No filename after >");
			return;
		}
		// else if outfile == "." || outfile == ".." || outfile.contains("/") {
		// 	println!("Error: Illegal filename after >");
		// 	return;

		// }
	}
	if redirectin{
		let temp: Vec<_> = commands[0].split("<").collect();
		let run = temp[0].trim();
		commands[0] = run;
		infile = temp[1].trim();
		if infile == ""{
			println!("Error: No filename after <");
			return;
		}
		// else if infile == "." || infile == ".." || infile.contains("/") {
		// 	println!("Error: Illegal filename after <");
		// 	return;

		// }
	}
	let mut processid = 0;
	for cmdid in 0..commands.len(){
		let (command, args) = parse(commands[cmdid]);
		if command.len() == 0{
			continue;
		}
		let mut call;
		match command.trim(){
			"cd" => {
				let tempd = env::current_dir().unwrap().to_str().unwrap().to_string();
				cd(args,&oldpwd);
				*oldpwd = tempd.clone();
				res = "".to_string();
				buildin = true;
				continue;
			},
	        "history" => {
	        	let mut out = String::new();
	        	for id in 1..history.len(){
					out += &format!("{:5}  {}\n",id,history[id-1]);
	        	}
	        	res = out;
				buildin = true;
				continue;
	        },
	        "jobs" => {
	        	jobs(history.clone(),joblist);
				res = "".to_string();
				buildin = true;
				continue;
	        }
	        "kill" => {
	        	kill(&args[1]);
				res = "".to_string();
				buildin = true;
				continue;
			},
	        "pwd" => {
	        	res = env::current_dir().unwrap().to_str().unwrap().to_string() + "\n" ;
				buildin = true;
	        	continue;
	        }
	        "exit" => {
	        	if commands.len() == 1{
	        		exit(0);
	        	}
	        	res = "".to_string();
				buildin = true;
	        	continue;
	        } 
			_ => {
				call = Command::new(command);
			}
		};
		for argid in 1..args.len(){
			call.arg(args[argid]);
		}
		if cmdid != 0{
			call.stdin(Stdio::piped());
		}
		if cmdid != commands.len()-1{
			call.stdout(Stdio::piped());
		}
		let mut run: Child;
		if !buildin && cmdid!=0{
			call.stdin(unsafe {Stdio::from_raw_fd(processes[processid-1].stdout.as_mut().unwrap().as_raw_fd()) });
		}
		if cmdid == commands.len()-1 && redirectout{
			let openres = File::create(Path::new(outfile));
			match openres{
				Ok(file) => {call.stdout(unsafe {Stdio::from_raw_fd(file.into_raw_fd())});},
				Err(e) => {println!("Error: {}",e); return;},
			}
			
		}
		if cmdid == 0 && redirectin{
			call.stdin(Stdio::piped());
			let openres = File::open(Path::new(infile));
			match openres{
				Ok(file) => {call.stdin(unsafe {Stdio::from_raw_fd(file.into_raw_fd())});},
				Err(e) => {println!("Error: {}",e); return;},
			}
		}
		run = call.spawn().unwrap();

		if buildin{
			run.stdin.as_mut().unwrap().write_all(res.as_bytes()).unwrap();
		}
		buildin = false;
		processes.push(run);
		processid += 1;
	}
	if !background{
		for id in 0..processes.len(){
			processes[id].wait().unwrap();
		}
	}
	if background{
		joblist.push(processes);
	}
	if buildin{
		print!("{}",res);
		stdout().flush().unwrap();
	}
}

fn main() {
    let mut history:Vec<String> = Vec::new();
    let mut jobpid:Vec<Vec<Child>> = Vec::new();
    let mut oldpwd = "Error: OLDPWD not set".to_string();
    loop {
    	print!("$ ");
	    stdout().flush().unwrap();
	    let mut input = String::new();
    	match stdin().read_line(&mut input) {
		    Ok(n) => {
		    	if n==0 {return;}
		    	input = input.replace("\r","").replace("\n","");
		    	history.push(input.clone());
		    	execute(&mut oldpwd, &mut jobpid, &history, &input);
		    }
		    Err(_) => return,
		}
    }
}