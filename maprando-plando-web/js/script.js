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