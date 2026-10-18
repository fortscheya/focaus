import { expect, test } from '@playwright/test'

async function nextSlide(page: import('@playwright/test').Page) {
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur())
  await page.getByRole('button', { name: 'Go to next slide' }).click()
  await page.waitForTimeout(450)
}

test('Focaus roadshow follows the six-slide interaction path', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('heading', { name: 'Focaus' })).toBeVisible()

  await nextSlide(page)
  await page.getByRole('button', { name: /Focaus.*一键跟上/ }).click()
  await expect(page.getByRole('heading', { name: 'Focaus 10s 认知对齐' })).toBeVisible()
  await page.getByRole('button', { name: '继续演示' }).click()
  await page.getByRole('button', { name: '继续演示' }).click()
  await expect(page.getByText('揭秘 Focaus 如何攻克')).toBeVisible()

  await nextSlide(page)
  await expect(page.getByRole('heading', { name: '资料先被理解，语音才有上下文' })).toBeVisible()
  await page.getByRole('button', { name: '步幅为二' }).click()
  await expect(page.getByText('建议修改为')).toBeVisible()
  await page.getByRole('button', { name: '一键替换正文' }).click()

  await nextSlide(page)
  await expect(page.getByRole('heading', { name: /笔记可以以不同的 level mix/ })).toBeVisible()
  await page.locator('input[aria-label="中英混合等级"]').fill('4')
  await expect(page.getByText('Level 4')).toBeVisible()
  await page.getByRole('button', { name: /一键添加至笔记/ }).click()
  await expect(page.getByText('自动生成：训练过程可视化')).toBeVisible()
  await expect(page.locator('canvas')).toBeVisible()

  await nextSlide(page)
  await expect(page.getByRole('heading', { name: '每个结论，都能回到它的事实来源' })).toBeVisible()
  await page.getByRole('button', { name: '原始转写' }).click()
  await page.getByRole('button', { name: 'ε' }).nth(0).click()
  await expect(page.getByText(/作用域：公式 1/)).toBeVisible()
  await page.getByRole('button', { name: 'ε' }).nth(1).click()
  await expect(page.getByText(/作用域：公式 2/)).toBeVisible()

  await nextSlide(page)
  await expect(page.getByRole('heading', { name: '一场路演结束，三层材料已经留下' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '本场路演 · 原始转写' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '本场路演 · 二次转写' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '本场路演 · 实时笔记' })).toBeVisible()

  const fitsViewport = await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1)
  expect(fitsViewport).toBe(true)
})
