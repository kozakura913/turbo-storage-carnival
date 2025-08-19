import { render as renderBlurhash } from "./buraha/index.js";

let popup_window=null;
window.addEventListener('click', () => {
	if(popup_window){
		document.body.removeChild(popup_window);
		popup_window=null;
	}
});
function embedView(entry){
	popup_window?.onclick();
	popup_window=document.createElement("div");
	popup_window.id="popup_window";
	document.body.appendChild(popup_window);
	let url="api/files/show/"+entry.id;
	if(entry.content_type?.startsWith("video/")){
		let video=document.createElement("video");
		video.src=url;
		video.width=entry.metadata?.width;
		video.height=entry.metadata?.height;
		video.controls=true;
		video.onclick=e=>e.stopPropagation();
		if(entry.blurhash)video.poster="api/files/show/"+entry.id+"?thumbnail=1";
		popup_window.appendChild(video);
	}else{
		let img=new Image();
		if(entry.content_type?.startsWith("image/")){
			img.src=url;
			img.width=entry.metadata?.width;
			img.height=entry.metadata?.height;
		}else{
			img.src="file.svg";
		}
		img.onclick=e=>e.stopPropagation();
		popup_window.appendChild(img);
	}
	let dl=document.createElement("a");
	dl.classList.add("download");
	dl.href=url;
	dl.innerText=i18n("download");
	dl.download=entry.name;
	dl.onclick=e=>e.stopPropagation();
	popup_window.appendChild(dl);
}
function menu(entry){
	let buttons=[];
	if(entry.name!=="../"){//親ディレクトリは削除と名前の変更ができない
		let del=document.createElement("a");
		del.innerText="削除";
		del.onclick=async()=>{
			let is_delete=confirm(entry.name+"\nこのファイルを削除しますか？");
			if(!is_delete)return;
			let res=await fetch("/api/files/delete",{
				method:"POST",
				body:JSON.stringify({
					id:entry.id,
				}),
				headers:{
					"Content-Type": "application/json"
				},
			});
			await loadFileList();
		}
		buttons.push(del);
		let mv=document.createElement("a");
		mv.innerText="移動";
		mv.onclick=async()=>{
			let new_name=prompt("新しい名前",entry.name);
			if(!new_name)return;
			let res=await fetch("/api/files/mv",{
				method:"POST",
				body:JSON.stringify({
					id:entry.id,
					name:new_name,
				}),
				headers:{
					"Content-Type": "application/json"
				},
			});
			await loadFileList();
		}
		buttons.push(mv);
	}
	popup_window?.onclick();
	if(buttons.length===0){
		return;
	}else{
		popup_window=document.createElement("dialog");
		popup_window.open=true;
		popup_window.id="popup_menu";
		popup_window.oncontextmenu=e=>{
			e.preventDefault();
		};
		document.body.appendChild(popup_window);
		for(let bt of buttons){
			popup_window.appendChild(bt);
		}
	}
}

let files=document.getElementById("file_list");
export async function loadFileList() {
	let directory=localStorage.getItem("directory");
	if(!directory){
		directory="/";
		localStorage.setItem("directory","/");
	}
	let since_id=0;
	files.innerHTML="";
	if(directory!=="/"){
		let s=directory.substring(0,directory.length-1);
		let t=s.substring(0,s.lastIndexOf("/"));
		load_entry({
			"blurhash": null,
			"content_type": "application/x-directory",
			"directory": t,
			"id": 120,
			"metadata": null,
			"name": "../",
			"sha256": null,
			"size": 0,
			"updated_at": "2023-03-11T23:26:14Z"
		});
	}
	for(let i=0;i<10;i++){
		let res=await fetch("/api/files/list",{
			method:"POST",
			body:JSON.stringify({
				directory,
				since_id,
			}),
			headers:{
				"Content-Type": "application/json"
			},
		});
		if(res.status==403){
			document.location="/login.html";
		}
		let json=await res.json();
		if(Array.isArray(json)){
			if(json.length===0){
				break;
			}
			for(let entry of json){
				await load_entry(entry);
				since_id=entry.id;
			}
		}
	}
}
async function load_entry(entry) {
	let file_icon=document.createElement("a");
	file_icon.id=entry.id;
	file_icon.classList.add("file_icon");
	let display_name=entry.name;
	if(entry.blurhash){
		let img=new Image();
		img.loading="lazy";
		img.src="api/files/show/"+entry.id+"?thumbnail=1";
		img.classList.add("thumbnail");
		file_icon.appendChild(img);
		img.width=entry.metadata?.width;
		img.height=entry.metadata?.height;
		let canvas=document.createElement("canvas");
		canvas.classList.add("blurhash");
		renderBlurhash(entry.blurhash,canvas);
		file_icon.appendChild(canvas);
	}else if(entry.name.endsWith("/")){
		let img=new Image();
		img.src="directory.svg";
		img.classList.add("thumbnail");
		file_icon.appendChild(img);
		display_name=entry.name.substring(0,entry.name.length-1);
	}else{
		let img=new Image();
		img.src="file.svg";
		img.classList.add("thumbnail");
		file_icon.appendChild(img);
	}
	file_icon.onclick=e=>{
		e.preventDefault();//動作握り潰し
		e.stopPropagation();
		if(entry.name.endsWith("/")){
			//ディレクトリを開く
			if(entry.name==="../"){
				localStorage.setItem("directory",entry.directory+"/");
			}else{
				localStorage.setItem("directory",entry.directory+entry.name);
			}
			loadFileList();
		}else{
			embedView(entry);
		}
	};
	if(entry.name.endsWith("/")){
		file_icon.addEventListener("dragenter", (event) => {
			if (event.dataTransfer.types.includes("application/x.tsc-file")) {
				event.preventDefault();
			}
		});
		file_icon.addEventListener("dragover", (event) => {
			if (event.dataTransfer.types.includes("application/x.tsc-file")) {
				event.preventDefault();
			}
		});
	}
	file_icon.draggable=true;
	file_icon.addEventListener("drop",async event=>{
		let id=event.dataTransfer.getData("application/x.tsc-file");
		if(!id){
			return;
		}
		let res=await fetch("/api/files/meta",{
			method:"POST",
			body:JSON.stringify({
				id:Number(event.target.parentElement.id),
			}),
			headers:{
				"Content-Type": "application/json"
			},
		});
		//TODO 親ディレクトリに移動する操作が機能しないので検証する
		let dir_info=await res.json();
		if(dir_info.name){
			let res=await fetch("/api/files/mv",{
				method:"POST",
				body:JSON.stringify({
					id:Number(id),
					directory:dir_info.directory+dir_info.name,
				}),
				headers:{
					"Content-Type": "application/json"
				},
			});
			loadFileList();
		}
	});
	file_icon.addEventListener("dragstart", (event) =>{
		event.dataTransfer.setData("application/x.tsc-file",entry.id);
		event.dataTransfer.effectAllowed = "move";
	});
	file_icon.href="api/files/show/"+entry.id;
	file_icon.oncontextmenu=e=>{
		e.preventDefault();
		menu(entry);
		popup_window.style.left = e.pageX + 'px';
		popup_window.style.top = e.pageY + 'px';
	}
	let file_name=document.createElement("a");
	file_name.classList.add("file_name");
	file_name.title=display_name;
	file_name.onclick=e=>file_icon.onclick(e);
	file_name.href="api/files/show/"+entry.id;
	file_name.innerText=display_name??i18n("no_title");
	let div=document.createElement("div");
	div.classList.add("file");
	div.appendChild(file_icon);
	div.appendChild(file_name);
	files.appendChild(div);
}
