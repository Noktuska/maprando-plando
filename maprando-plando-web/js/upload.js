async function submitSeed() {
    let submitBtn = document.getElementById("btn-upload-submit");
    submitBtn.disabled = true;
    openModal("upload-modal");
    const formData = new FormData(document.getElementById("upload-form"));
    const errDiv = document.getElementById("error-out");
    let response;
    try {
        response = await fetch("/upload-seed", {
            "method": "POST",
            "body": formData
        });
    } catch (e) {
        errDiv.textContent = `Error: ${e}`;
        closeModal();
        submitBtn.disabled = false;
        return;
    }

    if (response.ok) {
        let responseJson = await response.json();
        let seedUrl = responseJson["seed_id"];
        window.location.href = "seed/" + seedUrl;
    } else {
        let responseText = await response.text();
        errDiv.textContent = `Error status ${response.status}: ${responseText}`;
        closeModal();
        submitBtn.disabled = false;
    }
}

