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
        let seedUrl = responseJson["seed_id"];
        window.location.href = "seed/" + seedUrl;
    } else {
        let responseText = await response.text();
        errDiv.textContent = `Error status ${response.status}: ${responseText}`;
    }
}

function toggleDropdown(target) {
    const elem = document.getElementById(target);
    elem.classList.toggle("hidden");
}

function openModal(target) {
    const backdrop = document.getElementById("modal-backdrop");
    const elem = document.getElementById(target);
    if (elem !== null) {
        backdrop.classList.remove("hidden");
        elem.classList.remove("hidden");
    }
}

function closeModal() {
    const backdrop = document.getElementById("modal-backdrop");
    backdrop.classList.add("hidden");

    const modals = document.getElementsByClassName("modal");
    for (const modal of modals) {
        modal.classList.add("hidden");
    }
}

function swapButtonAssignment(elem) {
    const actions = ["control_shot", "control_jump", "control_dash", "control_item_select", "control_item_cancel", "control_angle_up", "control_angle_down"];
    const formEl = document.getElementById(elem.getAttribute("for"));
    const newButton = formEl.value;
    const oldButton = formEl.form.elements[formEl.name].value;
    for (const action of actions) {
        if (action == formEl.name) {
            continue;
        }
        if (formEl.form.elements[action].value == newButton) {
            formEl.form.elements[action].value = oldButton;
        }
    }
}

function selectSprite(sprite) {
    const input = document.getElementById("samusSprite");
    input.value = sprite.getAttribute("data-name");

    const imgPath = sprite.getElementsByClassName("sprite-image-static")[0].getAttribute("src");
    const displayName = sprite.getAttribute("data-display-name");

    const btn = document.getElementById("sprite-button");
    const img = btn.getElementsByTagName("img")[0];
    img.setAttribute("src", imgPath);
    const name = btn.getElementsByClassName("sprite-name")[0];
    name.textContent = displayName;

    saveForm(input.form);

    closeModal();
}

function updateSpriteDisplay() {
    let inputValue = document.getElementById("samusSprite").value;

    let display = document.getElementById("sprite-button");
    let displayImg = display.getElementsByTagName("img")[0];
    let displayName = display.getElementsByClassName("sprite-name")[0];

    let spriteList = document.getElementsByClassName("sprite");
    for (let spriteElem of spriteList) {
        let dataName = spriteElem.getAttribute("data-name");
        if (dataName != inputValue) {
            continue;
        }
        let spriteImg = spriteElem.getElementsByClassName("sprite-image-static")[0];
        displayImg.setAttribute("src", spriteImg.getAttribute("src"));
        displayName.textContent = spriteElem.getAttribute("data-display-name");
        break;
    }
}

document.getElementById("etank-button").addEventListener("click", function(ev) {
    document.getElementById("etank-picker").classList.remove("hidden");
    setPickerColor();
    ev.stopPropagation();
});

function closeColorPicker() {
    document.getElementById("etank-picker").classList.add("hidden");
}

let pickerColorH = 0;
let pickerColorS = 1;
let pickerColorV = 1;
let picker = document.getElementById("color-picker");
let slider = document.getElementById("color-slider");

function hsvToRgb() {
    let c = pickerColorV * pickerColorS;
    let x = c * (1 - Math.abs((pickerColorH / 60) % 2 - 1));
    let m = pickerColorV - c;
    let r, g, b;
    if (pickerColorH < 60) {
        [r, g, b] = [c, x, 0];
    } else if (pickerColorH < 120) {
        [r, g, b] = [x, c, 0];
    } else if (pickerColorH < 180) {
        [r, g, b] = [0, c, x];
    } else if (pickerColorH < 240) {
        [r, g, b] = [0, x, c];
    } else if (pickerColorH < 300) {
        [r, g, b] = [x, 0, c];
    } else {
        [r, g, b] = [c, 0, x];
    }
    [r, g, b] = [(r + m) * 255, (g + m) * 255, (b + m) * 255];
    r = Math.max(Math.min(r, 255), 0);
    g = Math.max(Math.min(g, 255), 0);
    b = Math.max(Math.min(b, 255), 0);
    return [r, g, b];
}

function rgbToHsv(r, g, b) {
    let max = Math.max(r, g, b);
    let min = Math.min(r, g, b);
    let v = max / 255;
    let s = 0;
    if (max !== 0) {
        s = (max - min) / max;
    }
    let h = 0;
    if (max - min != 0) {
        if (r >= g && r >= b) {
            h = 60 * (g - b) / (max - min);
        } else if (g >= r && g >= b) {
            h = 120 + 60 * (b - r) / (max - min);
        } else {
            h = 240 + 60 * (r - g) / (max - min);
        }
    }
    if (h < 0) {
        h += 360;
    }
    return [h, s, v];
}

function drawColorPicker() {
    let ctx = picker.getContext("2d");

    let sliderColor = `hsl(${pickerColorH}, 100%, 50%)`;
    let gradientH = ctx.createLinearGradient(0, 0, ctx.canvas.width, 0);
    gradientH.addColorStop(0, "#FFFFFF");
    gradientH.addColorStop(1, sliderColor);
    ctx.fillStyle = gradientH;
    ctx.fillRect(0, 0, ctx.canvas.width, ctx.canvas.height);

    let gradientV = ctx.createLinearGradient(0, 0, 0, ctx.canvas.height);
    gradientV.addColorStop(0, 'rgba(0, 0, 0, 0)');
    gradientV.addColorStop(1, '#000000');
    ctx.fillStyle = gradientV;
    ctx.fillRect(0, 0, ctx.canvas.width, ctx.canvas.height);
}

function setPickerColor() {
    let pickerColor = hsvToRgb();
    let pickerColorRgb = `rgb(${pickerColor[0]}, ${pickerColor[1]}, ${pickerColor[2]})`;
    let sliderColor = `hsl(${pickerColorH}, 100%, 50%)`;

    let pickerBbox = picker.getBoundingClientRect();
    let sliderBbox = slider.getBoundingClientRect();

    let pickerCursor = document.getElementById("color-picker-cursor");
    let pickerX = -10 + pickerColorS * (pickerBbox.right - pickerBbox.left);
    let pickerY = -10 + (1 - pickerColorV) * (pickerBbox.bottom - pickerBbox.top);
    pickerCursor.style.left = `${pickerX}px`;
    pickerCursor.style.top = `${pickerY}px`;
    pickerCursor.style.backgroundColor = pickerColorRgb;
    let sliderCursor = document.getElementById("color-slider-cursor");
    let sliderY = -6 + pickerColorH / 360 * (sliderBbox.bottom - sliderBbox.top);
    sliderCursor.style.top = `${sliderY}px`;
    sliderCursor.style.backgroundColor = sliderColor;

    let input = document.getElementById("etankColor");
    let arr = new Uint8Array(pickerColor);
    input.value = arr.toHex();
    saveForm(input.form);

    document.getElementById("etank-button").style.backgroundColor = pickerColorRgb;

    drawColorPicker();
}

function setPickerSV(x, y) {
    let bbox = picker.getBoundingClientRect();
    let relX = x - bbox.left;
    let relY = y - bbox.top;
    let xRatio = relX / (bbox.right - bbox.left);
    let yRatio = relY / (bbox.bottom - bbox.top);
    xRatio = Math.min(Math.max(xRatio, 0), 1);
    yRatio = Math.min(Math.max(yRatio, 0), 1);
    yRatio = 1 - yRatio; // Picker goes bottom to top
    pickerColorS = xRatio;
    pickerColorV = yRatio;
    setPickerColor();
}

function setPickerH(y) {
    let bbox = slider.getBoundingClientRect();
    let relY = y - bbox.top;
    let yRatio = relY / (bbox.bottom - bbox.top);
    yRatio = Math.min(Math.max(yRatio, 0), 1);
    pickerColorH = 360 * yRatio;
    setPickerColor();
}

function pickerValueChanged() {
    let newValue = document.getElementById("etankColor").value;
    let result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(newValue);
    if (result === null) {
        return;
    }
    let [r, g, b] = [parseInt(result[1], 16), parseInt(result[2], 16), parseInt(result[3], 16)];
    let [h, s, v] = rgbToHsv(r, g, b);
    pickerColorH = h;
    pickerColorS = s;
    pickerColorV = v;
    setPickerColor();
}

function colorPickerVanilla() {
    pickerColorH = 325;
    pickerColorS = 0.766;
    pickerColorV = 0.871;
    setPickerColor();
}

let isMovingPicker = false;
let isMovingSlider = false;

picker.addEventListener("mousedown", _ => isMovingPicker = true);
document.getElementById("color-picker-cursor").addEventListener("mousedown", _ => isMovingPicker = true);

slider.addEventListener("mousedown", _ => isMovingSlider = true);
document.getElementById("color-slider-cursor").addEventListener("mousedown", _ => isMovingSlider = true);

window.addEventListener("mousemove", function(ev) {
    if (isMovingPicker) {
        setPickerSV(ev.x, ev.y);
        ev.stopPropagation();
    }
    if (isMovingSlider) {
        setPickerH(ev.y);
        ev.stopPropagation();
    }
});
window.addEventListener("mouseup", function() {
    isMovingPicker = false;
    isMovingSlider = false;
});

function roomThemingChanged() {
    if (document.getElementById("roomThemingVanilla").checked) {
        document.getElementById("roomPalettesVanilla").checked = true;
        document.getElementById("tileTheme").value = "none"
    }
    if (document.getElementById("roomThemingPalettes").checked) {
        document.getElementById("roomPalettesAreaThemed").checked = true;
        document.getElementById("tileTheme").value = "none"
    }
    if (document.getElementById("roomThemingTiling").checked) {
        document.getElementById("roomPalettesVanilla").checked = true;
        document.getElementById("tileTheme").value = "area_themed"
    }
}
function roomThemingSettingChanged() {
    document.getElementById("roomThemingVanilla").checked = false;
    document.getElementById("roomThemingPalettes").checked = false;
    document.getElementById("roomThemingTiling").checked = false;
}

function saveForm(form) {
    let res = {};
    for (let elem of form.elements) {
        if (elem.type == "file" || elem.name == "") {
            continue;
        }
        if (elem.type == "radio" && !elem.checked) {
            if (res[elem.name] === undefined) {
                res[elem.name] = '';
            }
            continue;
        }
        if (elem.type == "checkbox") {
            res[elem.name] = elem.checked;
        } else {
            res[elem.name] = elem.value;
        }
    }
    localStorage[form.id] = JSON.stringify(res);
}

function loadForm(form) {
    if (localStorage[form.id] === undefined) {
        colorPickerVanilla();
        return false;
    }
    let data = JSON.parse(localStorage[form.id]);
    for (let elem of form.elements) {
        if (elem.type == "file" || elem.name == "") {
            continue;
        }
        if (elem.type == "radio") {
            let value = data[elem.name];
            if (value == elem.value) {
                elem.checked = true;
            } else if (value !== undefined) {
                elem.checked = false;
            }
        } else if (data[elem.name] !== undefined) {
            if (elem.type == "checkbox") {
                elem.checked = data[elem.name];
            } else {
                elem.value = data[elem.name];
            }
        }
    }
    updateSpriteDisplay();
    pickerValueChanged();
    return true;
}

loadForm(document.getElementById("seed-form"));

async function saveROM(field) {
    let file = field.files[0];
    const reader = new FileReader();
    reader.onload = async function() {
        try {
            await localforage.setItem("vanillaRomName", file.name);
            await localforage.setItem("vanillaRomData", reader.result);
        } catch (err){
            console.log(err);
        }
    };
    reader.readAsArrayBuffer(file);
}

async function loadROM() {
    const field = document.getElementById("rom");
    try {
        let fileName = await localforage.getItem("vanillaRomName");
        let data = await localforage.getItem("vanillaRomData");
        if (data !== null) {
            let file = new File([data], fileName, { type: '' });
            const dataTransfer = new DataTransfer();
            dataTransfer.items.add(file);
            field.files = dataTransfer.files;
        }
    } catch (err) {
        console.log(err);
    }
}

loadROM();

async function patchROM(form) {
    let romElem = document.getElementById("rom");
    if (romElem.value == "") {
        return false;
    }

    let romData = await localforage.getItem("vanillaRomData");
    let hashBuffer = await window.crypto.subtle.digest("SHA-256", romData);
    const hashArray = Array.from(new Uint8Array(hashBuffer));
    const hashHex = hashArray
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("");
    if (hashHex != "12b77c4bc9c1832cee8881244659065ee1d84c70c3d29e6eaf92e6798cc2ca72") {
        console.log("ROM Hash: " + hashHex);
        document.getElementById("romInvalid").classList.remove("hidden");
        return false;
    }

    form.submit();
}