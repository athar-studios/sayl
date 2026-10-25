// SAYL Download Manager - Popup Status Script (Firefox)

const HOST_NAME = "com.sayl.downloader";

document.addEventListener("DOMContentLoaded", () => {
  const statusDot = document.getElementById("status-dot");
  const statusAr = document.getElementById("status-ar");
  const statusEn = document.getElementById("status-en");
  const hostDetails = document.getElementById("host-details");
  const recheckBtn = document.getElementById("recheck-btn");

  async function checkHost() {
    statusDot.className = "dot pending";
    statusAr.textContent = "جاري فحص الاتصال بالمضيف...";
    statusEn.textContent = "Checking native host connection...";
    hostDetails.classList.add("hidden");
    hostDetails.textContent = "";

    try {
      const response = await browser.runtime.sendNativeMessage(HOST_NAME, { type: "Ping", version: 1 });
      if (response && response.type === "Pong") {
        statusDot.className = "dot success";
        statusAr.textContent = "المضيف متصل وجاهز";
        statusEn.textContent = "Host connected and ready";
        hostDetails.textContent = `SAYL v${response.app_version} (Protocol v${response.version})`;
        hostDetails.classList.remove("hidden");
      } else if (response && response.type === "Error") {
        statusDot.className = "dot error";
        statusAr.textContent = "خطأ من المضيف";
        statusEn.textContent = `Host error: ${response.code}`;
        hostDetails.textContent = response.message || "Unknown error";
        hostDetails.classList.remove("hidden");
      } else {
        statusDot.className = "dot error";
        statusAr.textContent = "استجابة غير متوقعة";
        statusEn.textContent = "Unexpected host response";
        hostDetails.classList.remove("hidden");
      }
    } catch (err) {
      statusDot.className = "dot error";
      statusAr.textContent = "المضيف غير متصل";
      statusEn.textContent = "Host not connected";
      hostDetails.textContent = (err && err.message) || "Failed to reach native messaging host";
      hostDetails.classList.remove("hidden");
    }
  }

  if (recheckBtn) {
    recheckBtn.addEventListener("click", checkHost);
  }

  checkHost();
});
