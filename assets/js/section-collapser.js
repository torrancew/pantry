async function wrap(heading, children, klass = null) {
  const wrapper = document.createElement("details");
  wrapper.setAttribute("open", true);
  if (klass != null) {
    wrapper.classList.add(klass);
  }

  const summary = document.createElement("summary");

  wrapper.appendChild(summary);
  for (const child of children) {
    wrapper.appendChild(child);
  }

  heading.parentNode.insertBefore(wrapper, heading);
  summary.appendChild(heading);
}

async function wrapSection(ident) {
  const heading = document.querySelector(`h2#${ident}`);
  const unified_list = document.querySelector(`h2#${ident} + ul`);

  if (unified_list != null) {
    wrap(heading, [unified_list], ident);
  } else {
    document.querySelectorAll(`h3.${ident}`).forEach((item) => {
      const list = item.nextElementSibling;
      wrap(item, [list], ident);
    });

    const sub_sections = document.querySelectorAll(`details.${ident}`);
    wrap(heading, sub_sections, ident);
  }
}

async function modifyLists() {
  await wrapSection('ingredients');
  await wrapSection('directions');
}

document.addEventListener("DOMContentLoaded", modifyLists);
// vim set: ts=4 sts=4 sw=4 et:
