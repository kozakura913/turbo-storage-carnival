"use strict";

async function upload(file){
	let directory=localStorage.getItem("directory");
	if(!directory){
		directory="/";
		localStorage.setItem("directory","/");
	}
	let split_size=10*1024*1024;//10MBチャンク
	let content_length=file.size;
	const preflight_status = await fetch('/api/files/upload/preflight', {
		method: 'POST',
		body: JSON.stringify({
			content_length,
			directory,
			title: file.name,
			last_modified:(new Date(file.lastModified)).toISOString(),
		}),
		headers:{
			"Content-Type": "application/json"
		},
	});
	console.log(preflight_status);
	const json = await preflight_status.json();
	const session_id = json.session_id;//upload-serviceサーバーが処理を管理するためのID。S3側とは無関係に振られる
	let part_number = -1;//part_numberは0から振る
	let offset = 0;//ファイルのどこから送信するべきか
	let file_upload=document.getElementById("file_upload");
	file_upload.max = content_length;//後で進捗バー追加する
	while (offset < content_length) {
		part_number++;
		const part_blob = file.slice(offset, offset + split_size);
		const upload_status_code = await new Promise((resolve) => {
			const xhr = new XMLHttpRequest();
			xhr.onreadystatechange = function() {
				if (xhr.readyState === 4) {
					resolve(xhr.status);
				}
			};
			xhr.open('POST', '/api/files/upload/partial?part=' + part_number, true);
			xhr.setRequestHeader('Authorization', 'Bearer ' + session_id);
			xhr.upload.onprogress = ev => {
				if (ev.lengthComputable) {
					file_upload.value=offset + ev.loaded;
					//ctx.progressValue = offset + ev.loaded;//後で進捗バー追加する
				}
			};
			xhr.send(part_blob);
		});
		if (upload_status_code < 200 || upload_status_code >= 300) {
			const wip = await fetch('/api/files/upload/abort', {
				method: 'POST',
				headers: {
					Authorization: 'Bearer ' + session_id,
					"Content-Type": "application/json"
				},
				body: JSON.stringify({
					part_length: part_number,
				}),
			});
			alert({
				type: 'error',
				title: i18n.ts.failedToUpload,
				text: `multipart part${part_number}`,
			});
			break;
		}
		offset += split_size;
	}
	file_upload.value=file_upload.max;
	const drive_file = await fetch('/api/files/upload/finish', {
		method: 'POST',
		headers: {
			Authorization: 'Bearer ' + session_id,
			"Content-Type": "application/json",
		},
		body: JSON.stringify({
		}),
	});
	file_upload.value=0;
	let mod=await import("./file-list.js");
	await mod.loadFileList();
}
