import { expect, test } from '@playwright/test'

// The browser build boots against the in-memory mock transport with the
// "Acme API" fixture workspace (incl. a 1000-node folder).

test.beforeEach(async ({ page }) => {
  await page.goto('/')
  await expect(page.getByText('Acme API')).toBeVisible()
})

test('boot: tree renders and virtualizes the 1000-node folder', async ({ page }) => {
  const tree = page.getByRole('tree')
  await expect(tree.getByText('Health check')).toBeVisible()
  await expect(tree.getByText('Users')).toBeVisible()

  // expand the big folder — virtualization keeps the DOM small
  await tree.getByText('Generated (1000)').click()
  await expect(tree.getByText('Generated 0000')).toBeVisible()
  const renderedRows = await page.getByRole('treeitem').count()
  expect(renderedRows).toBeLessThan(120)

  // filtering narrows the tree
  await page.getByLabel('Filter requests…').fill('Health')
  await expect(tree.getByText('Health check')).toBeVisible()
  await expect(tree.getByText('Users')).toBeHidden()
  await page.getByLabel('Filter requests…').fill('')
})

test('open request in a tab and send it against the mock', async ({ page }) => {
  const tree = page.getByRole('tree')
  await tree.getByText('Users').click()
  await tree.getByText('Create user').dblclick()

  // tab opened with the request title
  const tab = page.getByRole('tab', { name: /Create user/ })
  await expect(tab).toBeVisible()

  // send -> mock responds 200 with timing/size chrome
  await page.getByRole('button', { name: 'Send' }).click()
  await expect(page.getByText(/200/).first()).toBeVisible()
  await expect(page.getByText(/ms/).first()).toBeVisible()
})

test('node CRUD: create, rename via F2, delete via context menu', async ({ page }) => {
  const tree = page.getByRole('tree')

  await page.getByLabel('New request').click()
  await expect(tree.getByText('New request')).toBeVisible()

  // rename inline with F2 (controlled input -> locate structurally)
  await tree.getByText('New request').click()
  await page.keyboard.press('F2')
  const rename = tree.locator('input')
  await expect(rename).toBeVisible()
  await rename.fill('Ping endpoint')
  await rename.press('Enter')
  await expect(tree.getByText('Ping endpoint')).toBeVisible()

  // delete via context menu
  await tree.getByText('Ping endpoint').click({ button: 'right' })
  await page.getByRole('menuitem', { name: 'Delete' }).click()
  await expect(tree.getByText('Ping endpoint')).toBeHidden()
})

test('tabs: preview replacement, promotion, dirty dot, close and reopen', async ({ page }) => {
  const tree = page.getByRole('tree')

  // single click = preview (italic, replaced by next preview)
  await tree.getByText('Health check').click()
  await expect(page.getByRole('tab', { name: /Health check/ })).toBeVisible()

  await tree.getByText('Users').click()
  await tree.getByText('List users').click()
  await expect(page.getByRole('tab', { name: /List users/ })).toBeVisible()
  await expect(page.getByRole('tab', { name: /Health check/ })).toBeHidden()

  // edit -> dirty dot appears and the preview is promoted
  const url = page.getByLabel('Request URL')
  await url.fill('https://httpbin.org/anything/users?edited=1')
  await expect(page.getByRole('tab', { name: /List users/ }).getByLabel('unsaved')).toBeVisible()

  // a new preview now coexists (promoted tab stays)
  await tree.getByText('Health check').click()
  await expect(page.getByRole('tab', { name: /List users/ })).toBeVisible()
  await expect(page.getByRole('tab', { name: /Health check/ })).toBeVisible()

  // save via ctrl+s clears the dot
  await page.getByRole('tab', { name: /List users/ }).click()
  await page.keyboard.press('Control+s')
  await expect(page.getByRole('tab', { name: /List users/ }).getByLabel('unsaved')).toBeHidden()

  // ctrl+w closes, ctrl+shift+t reopens
  await page.keyboard.press('Control+w')
  await expect(page.getByRole('tab', { name: /List users/ })).toBeHidden()
  await page.keyboard.press('Control+Shift+t')
  await expect(page.getByRole('tab', { name: /List users/ })).toBeVisible()
})

test('gallery route renders both themes for visual checks', async ({ page }) => {
  await page.goto('/#/gallery')
  await expect(page.getByTestId('gallery')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Send', exact: true })).toBeVisible()
  await page.getByLabel('Toggle dark theme').click()
  await expect(page.locator('html')).toHaveClass(/dark/)
})
