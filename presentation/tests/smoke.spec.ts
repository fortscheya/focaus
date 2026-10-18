import { expect, test } from '@playwright/test'

async function next(page: import('@playwright/test').Page) {
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur())
  await page.getByRole('button', { name: 'Go to next slide', exact: true }).click()
  await page.waitForTimeout(650)
}

test('eight-slide story, replay, source navigation and calculations work with mouse input', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('heading', {name: 'Focaus', exact:true})).toBeVisible()
  await next(page)
  await page.getByRole('button', {name:'彩排回放',exact:true}).click()
  await page.getByRole('button', {name:'开始彩排回放',exact:true}).click()
  await expect(page.getByText('partial · 尚未定稿', {exact:true})).toBeVisible()
  await expect(page.locator('.asr-transcript-row.is-final')).toHaveCount(1, {timeout: 5000})
  await page.getByRole('button', {name:/摘录句子/}).first().click()
  await next(page)
  await expect(page.getByRole('heading',{name:'原话留下，关键订正由你决定。'})).toBeVisible()
  await page.getByRole('button', {name:/Sigmoid 的输出是以零为中心的/}).click()
  await expect(page.getByRole('dialog', {name:'订正建议'})).toBeVisible()
  await page.getByRole('button', {name:'采纳订正',exact:true}).click()
  await expect(page.getByText('已采纳 1 处',{exact:true})).toBeVisible()
  await expect(page.locator('.raw-timeline')).toContainText('它不是 zero-centered')
  await next(page)
  await page.getByRole('button', {name:'展开听写链',exact:true}).click()
  await page.getByRole('button', {name:'展开完整工作区',exact:true}).click()
  await expect(page.locator('.workspace-pane:visible')).toHaveCount(6)
  await next(page)
  await page.getByRole('button', {name:'focaus · 回顾刚刚十分钟',exact:true}).click()
  await page.getByRole('button', {name:'跟上当前位置 →',exact:true}).click()
  await expect(page.getByRole('button', {name:'已跟上当前位置 ✓'})).toBeVisible()
  await expect(page.locator('.classroom-note')).toContainText('Xavier')
  await next(page)
  await page.getByRole('button', {name:/原始句 148/}).click()
  await expect(page.locator('.raw-quote:visible')).toContainText('它不是 zero-centered')
  await page.getByRole('button', {name:/课件 p.35/}).click()
  await expect(page.locator('.evidence-page:visible')).toBeVisible()
  await next(page)
  await page.getByRole('button', {name:'选择输出 2,2',exact:true}).click()
  await page.getByRole('button', {name:'下一项乘加',exact:true}).click()
  await expect(page.locator('.lab-step-bar')).toContainText('输入 (2, 2)')
  await expect(page.locator('.inspector-value strong')).toHaveText('4')
  await next(page)
  await expect(page.getByRole('heading', {name:'本场摘录',exact:true})).toBeVisible()
  await expect(page.locator('.closing-system-card').last()).toContainText('来源：本场')
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1)).toBe(true)
})

test('live ASR uses provider events only; stopping preserves the final tail and releases audio', async ({page}) => {
  await page.addInitScript(() => {
    const stopped = () => { (window as any).__audioReleased = true }
    Object.defineProperty(navigator, 'mediaDevices', {value:{getUserMedia:async()=>({getTracks:()=>[{stop:stopped}]})}})
    const connection = () => ({connect(){},disconnect(){}})
    ;(window as any).AudioContext = class {
      state = 'running'; destination = {}; audioWorklet = {addModule:async()=>undefined}
      createMediaStreamSource(){return connection()}
      createGain(){return {...connection(),gain:{value:0}}}
      async resume(){}
      async close(){this.state='closed'}
    }
    ;(window as any).AudioWorkletNode = class {
      port = {onmessage:null as any,postMessage:()=>this.port.onmessage?.({data:{type:'flushed'}})}
      connect(){} disconnect(){}
    }
  })
  await page.route('**/api/demo/status', route => route.fulfill({json:{configured:true,remainingSeconds:100},headers:{'access-control-allow-origin':'*'}}))
  await page.routeWebSocket('**/api/demo/asr', ws => {
    ws.onMessage(message => {
      if (typeof message !== 'string') return
      const control = JSON.parse(message)
      if (control.type === 'start') {
        ws.send(JSON.stringify({type:'ready'}))
        ws.send(JSON.stringify({type:'partial',text:'这句是协议验证'}))
        setTimeout(()=>ws.send(JSON.stringify({type:'final',segmentId:'real-test-1',text:'这句是协议验证返回，不是预置课堂句。',startMs:200,endMs:1400})),500)
      }
      if (control.type === 'stop') {
        ws.send(JSON.stringify({type:'final',segmentId:'tail',text:'停止后的尾句。',startMs:1500,endMs:1800}))
        ws.send(JSON.stringify({type:'stopped'}))
      }
    })
  })
  await page.goto('/2')
  await page.getByRole('button', {name:'开始实时识别',exact:true}).click()
  await expect(page.locator('.asr-transcript')).toContainText('这句是协议验证返回，不是预置课堂句。')
  await expect(page.locator('.asr-transcript')).not.toContainText('stuck')
  await page.getByRole('button', {name:'停止识别',exact:true}).click()
  await expect(page.locator('.asr-transcript')).toContainText('停止后的尾句。')
  await expect(page.getByRole('button', {name:'开始实时识别',exact:true})).toBeVisible()
  expect(await page.evaluate(()=>(window as any).__audioReleased)).toBe(true)
})

test('unavailable host reports an error without falling back to fake recognition',async({page})=>{
  await page.route('**/api/demo/status',route=>route.fulfill({status:503,json:{},headers:{'access-control-allow-origin':'*'}}))
  await page.goto('/2')
  await page.getByRole('button',{name:'开始实时识别',exact:true}).click()
  await expect(page.getByRole('alert')).toContainText('语音宿主暂不可达')
  await expect(page.locator('.asr-transcript-row.is-final')).toHaveCount(0)
})

test('cancelling before the status response cannot open the microphone later',async({page})=>{
  await page.addInitScript(()=>{
    ;(window as any).__micCalls = 0
    Object.defineProperty(navigator,'mediaDevices',{value:{getUserMedia:async()=>{(window as any).__micCalls++;throw new Error('must not run')}}})
  })
  let releaseStatus!: () => void
  const statusHeld = new Promise<void>(resolve => { releaseStatus = resolve })
  await page.route('**/api/demo/status',async route=>{
    await statusHeld
    await route.fulfill({json:{configured:true,remainingSeconds:100},headers:{'access-control-allow-origin':'*'}})
  })
  await page.goto('/2')
  const statusRequested = page.waitForRequest('**/api/demo/status')
  await page.getByRole('button',{name:'开始实时识别',exact:true}).click()
  await statusRequested
  await page.getByRole('button',{name:'停止识别',exact:true}).click()
  const statusReturned = page.waitForResponse('**/api/demo/status')
  releaseStatus()
  await statusReturned
  await page.waitForTimeout(200)
  expect(await page.evaluate(()=>(window as any).__micCalls)).toBe(0)
  await expect(page.getByRole('button',{name:'开始实时识别',exact:true})).toBeVisible()
})
