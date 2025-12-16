async function submitSeed() {
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
        return;
    }

    if (response.ok) {
        let responseJson = await response.json();
        let seedUrl = responseJson["seed_url"];
        window.location.href = seedUrl;
    } else {
        let responseText = await response.text();
        errDiv.textContent = `Error status ${response.status}: ${responseText}`;
    }
}