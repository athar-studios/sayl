import asyncio
from playwright.async_api import async_playwright
async def main():
  async with async_playwright() as p:
    b=await p.chromium.launch(args=["--disable-background-timer-throttling","--disable-renderer-backgrounding"])
    c=await b.new_context(viewport={"width":1440,"height":900},record_video_dir="vid",record_video_size={"width":1440,"height":900})
    x=await c.new_page();await x.goto("file:///workspace/sayl-site-concept/index.html");await x.wait_for_timeout(7500);
    r=await x.eval_on_selector("#logo","e=>{const b=e.getBoundingClientRect();return [b.x,b.y,b.width,b.height]}");print(r);await c.close();await b.close()
asyncio.run(main())
