function handleClick(ev) {
  this.classList.toggle("checked");
}

function attachHandler(el) {
  el.addEventListener("click", handleClick);
}

async function attachClickHandlers() {
  const flat_lists = document.querySelectorAll("h2 + ul li");
  flat_lists.forEach(attachHandler);

  const nested_lists = document.querySelectorAll("h3 + ul li");
  nested_lists.forEach(attachHandler);
}

document.addEventListener("DOMContentLoaded", attachClickHandlers);
// vim set: ts=4 sts=4 sw=4 et:
