import asyncio
from playwright.async_api import async_playwright
async def main():
  async with async_playwright() as p:
    b=await p.chromium.launch()
    for name,vp,mob in [("desktop",{"width":1440,"height":900},False),("mobile",{"width":390,"height":844},True)]:
      for pg in ["index","docs"]:
        c=await b.new_context(viewport=vp,device_scale_factor=1,is_mobile=mob);x=await c.new_page()
        await x.goto(f"file:///workspace/sayl-site-concept/{pg}.html");await x.wait_for_timeout(4000)
        await x.screenshot(path=f"shots/{pg}-{name}.png");await x.screenshot(path=f"shots/{pg}-{name}-full.png",full_page=True);await c.close()
    await b.close()
asyncio.run(main())
