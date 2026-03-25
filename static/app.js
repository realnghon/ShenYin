const LARGE_TEXT_BUFFER_THRESHOLD = 200_000;

const state = {
  buffers: new Map(),
};

function qs(selector, scope = document) {
  return scope.querySelector(selector);
}

function qsa(selector, scope = document) {
  return [...scope.querySelectorAll(selector)];
}

function escapeHtml(value) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function formatBytes(bytes) {
  if (!Number.isFinite(bytes)) {
    return "";
  }
  const units = ["B", "KB", "MB", "GB"];
  let index = 0;
  let size = bytes;
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024;
    index += 1;
  }
  return `${size.toFixed(size >= 10 || index === 0 ? 0 : 1)} ${units[index]}`;
}

function utf8ByteLength(text) {
  return new TextEncoder().encode(text).length;
}

function renderMessage(target, type, text) {
  target.innerHTML = `<div class="${type === "error" ? "error-box" : "notice"}">${escapeHtml(text)}</div>`;
}

function toggleModeBlocks(form) {
  const inputType = qs('input[name="input_type"]:checked', form)?.value ?? "text";

  qsa(".text-only", form).forEach((node) => node.classList.toggle("hidden", inputType !== "text"));
  qsa(".file-only", form).forEach((node) => node.classList.toggle("hidden", inputType !== "file"));
}

function syncEncryptDefaults(form) {
  if (form.id !== "encrypt-form") {
    return;
  }

  const inputType = qs('input[name="input_type"]:checked', form)?.value ?? "text";
  const outputFormat = qs("[data-output-format]", form);
  const hint = qs("#encrypt-mode-hint");

  if (!outputFormat || !hint) {
    return;
  }

  if (inputType === "text") {
    outputFormat.value = "armor";
    hint.textContent = "当前是文本输入，会返回文本结果；过长时只提供下载。";
    return;
  }

  if (outputFormat.value === "armor") {
    hint.textContent = "当前是文件输入 + 文本结果，可直接复制或下载。";
    return;
  }

  hint.textContent = "当前是文件输入 + 文件结果，适合直接下载保存。";
}

function renderResult(target, payload, message) {
  target.innerHTML = "";

  const block = document.createElement("div");
  block.className = "result-block";

  const notice = document.createElement("div");
  notice.className = "notice";
  notice.textContent = message;
  block.appendChild(notice);

  const metaParts = [];
  if (payload.filename) {
    metaParts.push(`文件名：${payload.filename}`);
  }
  if (Number.isFinite(payload.size)) {
    metaParts.push(`大小：${formatBytes(payload.size)}`);
  }
  if (metaParts.length) {
    const meta = document.createElement("div");
    meta.className = "result-meta";
    meta.textContent = metaParts.join(" | ");
    block.appendChild(meta);
  }

  if (payload.text_too_large) {
    const preview = document.createElement("div");
    preview.className = "notice";
    preview.textContent = `文本结果较大（约 ${formatBytes(payload.text_length)}），页面不直接展开。请使用下载按钮获取完整文本。`;
    block.appendChild(preview);
  } else if (payload.text_available && payload.text) {
    const textarea = document.createElement("textarea");
    textarea.className = "result-output";
    textarea.readOnly = true;
    textarea.spellcheck = false;
    textarea.value = payload.text;
    block.appendChild(textarea);
  }

  const actions = document.createElement("div");
  actions.className = "result-actions";

  if (payload.text_available && payload.text) {
    const copyButton = document.createElement("button");
    copyButton.type = "button";
    copyButton.className = "mini-btn";
    copyButton.dataset.copyResult = "";
    copyButton.textContent = "复制结果文本";
    actions.appendChild(copyButton);
  }

  if (payload.download_url) {
    const link = document.createElement("a");
    link.className = "download-link";
    link.href = payload.download_url;
    link.target = "_blank";
    link.rel = "noreferrer";
    link.textContent = `下载 ${payload.filename}`;
    actions.appendChild(link);
  }

  if (actions.childNodes.length) {
    block.appendChild(actions);
  }

  target.appendChild(block);
}

async function parseResponse(response) {
  const contentType = response.headers.get("content-type") ?? "";

  if (!contentType.includes("application/json")) {
    const text = await response.text();
    const snippet = text.replace(/\s+/g, " ").slice(0, 160);
    throw new Error(`接口返回了非 JSON 响应（${response.status}）。${snippet || "请查看服务端日志。"}`);
  }

  const data = await response.json();
  if (!response.ok || !data.ok) {
    throw new Error(data.message || `请求失败（${response.status}）`);
  }
  return data;
}

function bufferKey(form, fieldName) {
  return `${form.id}:${fieldName}`;
}

function getBufferedText(form, fieldName) {
  return state.buffers.get(bufferKey(form, fieldName)) ?? "";
}

function setBufferedText(form, fieldName, text) {
  if (text) {
    state.buffers.set(bufferKey(form, fieldName), text);
  } else {
    state.buffers.delete(bufferKey(form, fieldName));
  }
  updateBufferedStatus(form, fieldName);
}

function clearBufferedText(form, fieldName) {
  setBufferedText(form, fieldName, "");
}

function updateBufferedStatus(form, fieldName) {
  const textarea = qs(`[name="${fieldName}"]`, form);
  const status = qs(`[data-buffer-status="${fieldName}"]`, form);
  const clearButton = qs(`[data-buffer-clear="${fieldName}"]`, form);
  if (!textarea || !status || !clearButton) {
    return;
  }

  const bufferedText = getBufferedText(form, fieldName);
  if (!bufferedText) {
    status.textContent = "";
    status.classList.add("hidden");
    clearButton.classList.add("hidden");
    return;
  }

  const active = !textarea.value;
  status.textContent = active
    ? `已接收超长文本，约 ${formatBytes(utf8ByteLength(bufferedText))}。页面不展开，提交时会直接使用缓存内容。`
    : `已保留一份超长文本缓存，约 ${formatBytes(utf8ByteLength(bufferedText))}。当前输入框有可见内容，提交时会优先使用输入框。`;
  status.classList.remove("hidden");
  clearButton.classList.remove("hidden");
}

function bindLargePasteBuffer(form, textarea) {
  const fieldName = textarea.name;
  const clearButton = qs(`[data-buffer-clear="${fieldName}"]`, form);

  textarea.addEventListener("paste", (event) => {
    const text = event.clipboardData?.getData("text") ?? "";
    if (text.length < LARGE_TEXT_BUFFER_THRESHOLD) {
      if (getBufferedText(form, fieldName)) {
        window.setTimeout(() => clearBufferedText(form, fieldName), 0);
      }
      return;
    }

    event.preventDefault();
    textarea.value = "";
    setBufferedText(form, fieldName, text);
  });

  textarea.addEventListener("input", () => updateBufferedStatus(form, fieldName));

  if (clearButton) {
    clearButton.addEventListener("click", () => {
      textarea.value = "";
      clearBufferedText(form, fieldName);
      textarea.focus();
    });
  }

  updateBufferedStatus(form, fieldName);
}

function buildFormData(form) {
  const formData = new FormData(form);

  qsa("[data-buffer-upload-field]", form).forEach((textarea) => {
    const bufferedText = getBufferedText(form, textarea.name);
    if (!bufferedText || textarea.value) {
      return;
    }

    const uploadField = textarea.dataset.bufferUploadField;
    const uploadName = textarea.dataset.bufferFilename || `${textarea.name}.txt`;
    const blob = new Blob([bufferedText], { type: "text/plain;charset=utf-8" });

    formData.delete(textarea.name);
    formData.set(uploadField, blob, uploadName);
  });

  return formData;
}

async function submitForm(form, url, resultTarget) {
  const response = await fetch(url, {
    method: "POST",
    body: buildFormData(form),
  });
  const data = await parseResponse(response);
  if (resultTarget) {
    renderResult(resultTarget, data.result, data.message);
  }
  return data;
}

function bindToolForm(formId, url, resultId) {
  const form = qs(formId);
  const resultTarget = qs(resultId);

  toggleModeBlocks(form);
  syncEncryptDefaults(form);
  qsa("[data-buffer-upload-field]", form).forEach((textarea) => bindLargePasteBuffer(form, textarea));

  form.addEventListener("change", () => toggleModeBlocks(form));
  qsa('input[name="input_type"]', form).forEach((node) => {
    node.addEventListener("change", () => syncEncryptDefaults(form));
  });
  qsa("[data-output-format]", form).forEach((node) => {
    node.addEventListener("change", () => syncEncryptDefaults(form));
  });

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    renderMessage(resultTarget, "notice", "处理中，请稍候…");
    try {
      await submitForm(form, url, resultTarget);
    } catch (error) {
      renderMessage(resultTarget, "error", error.message);
    }
  });
}

function bindGlobalEvents() {
  document.addEventListener("click", async (event) => {
    const copyButton = event.target.closest("[data-copy-result]");
    if (copyButton) {
      const textarea = copyButton.closest(".result-block")?.querySelector(".result-output");
      if (!textarea) {
        return;
      }
      await navigator.clipboard.writeText(textarea.value);
      copyButton.textContent = "已复制";
      window.setTimeout(() => {
        copyButton.textContent = "复制结果文本";
      }, 1200);
    }
  });
}

function init() {
  bindToolForm("#encrypt-form", "/api/encrypt", "#encrypt-result");
  bindToolForm("#decrypt-form", "/api/decrypt", "#decrypt-result");
  bindGlobalEvents();
}

window.addEventListener("DOMContentLoaded", init);
