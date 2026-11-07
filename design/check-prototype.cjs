const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { chromium } = require(process.env.ALTRUN_PLAYWRIGHT_MODULE || 'playwright');
const root = path.resolve(__dirname, '..');
const out = path.join(__dirname, 'qa');
fs.mkdirSync(out, {recursive:true});
fs.mkdirSync(path.join(out,'temp'), {recursive:true});
process.env.TEMP=path.join(out,'temp');process.env.TMP=path.join(out,'temp');
const cases=[];
const pass=s=>cases.push(s);
(async()=>{
  const browser=await chromium.launch({executablePath:'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',headless:true,args:['--no-first-run','--disable-background-networking']});
  try{
    const page=await browser.newPage({viewport:{width:1024,height:850},colorScheme:'light'});
    const errors=[];page.on('pageerror',e=>errors.push(e.message));
    await page.setContent(fs.readFileSync(path.join(__dirname,'altrun-prototype.html'),'utf8'));
    const q=page.locator('#ar-query');
    assert.equal(await page.locator('.ar-result').count(),10);pass('首屏十条原版常用快捷项');
    const compact=await page.locator('.ar-launcher').boundingBox();assert.ok(compact.width<=420&&compact.height<=300,JSON.stringify(compact));
    assert.equal(await page.locator('.ar-result .ar-app-icon,.ar-result .ar-detail,.ar-result .ar-kind').count(),0);
    assert.match(await page.locator('#ar-selected-name').innerText(),/我的电脑/);assert.match(await page.locator('#ar-command').inputValue(),/^CMD=/);
    await page.locator('.ar-result').nth(3).click();assert.equal(await page.locator('#ar-selected-name').innerText(),'记事本');assert.equal(await page.locator('#ar-status').innerText(),'');
    await q.press('ArrowDown');assert.equal(await page.locator('#ar-selected-name').innerText(),'画图');
    await q.press('Control+4');assert.match(await page.locator('#ar-status').innerText(),/记事本/);await q.fill('');pass('420px 紧凑尺寸、原版元素、单击选中和数字执行');
    await page.screenshot({path:path.join(out,'launcher-light.png'),fullPage:true});
    await q.fill('g');assert.equal(await page.locator('.ar-result').count(),2);await q.press('Enter');
    assert.equal(await page.locator('#ar-param-dialog').isVisible(),true);
    await page.locator('#ar-param-form button[type=submit]').click();assert.match(await page.locator('#ar-param-error').innerText(),/请输入/);
    await page.locator('#ar-param').fill('你好 Windows');assert.match(await page.locator('#ar-param-preview').innerText(),/%E4%BD%A0/);
    await page.screenshot({path:path.join(out,'parameter.png'),fullPage:true});
    await page.locator('#ar-param-form button[type=submit]').click();assert.match(await page.locator('#ar-status').innerText(),/模拟执行/);pass('参数弹层、错误及模拟执行');
    await q.fill('notepad');assert.equal(await page.locator('.ar-result').count(),1);const one=await page.locator('.ar-launcher').boundingBox();assert.ok(one.height<130,JSON.stringify(one));
    await q.fill('不存在的命令');assert.match(await page.locator('#ar-results').innerText(),/没有匹配/);await page.locator('#ar-clear').click();assert.equal(await q.inputValue(),'');pass('结果自然收缩、无结果及清空');
    await q.dispatchEvent('compositionstart');await q.fill('g');await q.dispatchEvent('keydown',{key:'Enter',isComposing:true});assert.equal(await page.locator('#ar-param-dialog').isVisible(),false);await q.dispatchEvent('compositionend');pass('合成输入事件不会提前执行（非真实 IME 测试）');
    await page.locator('[data-view=manager]').click();await page.locator('#ar-new').click();await page.locator('#ar-editor button[type=submit]').click();assert.match(await page.locator('#ar-edit-error').innerText(),/请填写/);
    await page.locator('#ar-alias').fill('notepad');await page.locator('#ar-name').fill('重复测试');await page.locator('#ar-cmd').fill('test.exe');await page.locator('#ar-editor button[type=submit]').click();assert.match(await page.locator('#ar-edit-error').innerText(),/已存在/);
    await page.locator('#ar-alias').fill('new');await page.locator('#ar-editor button[type=submit]').click();assert.equal(await page.locator('#ar-table tr').count(),11);pass('新建、必填和重复快捷字校验');
    await page.locator('#ar-name').fill('未保存测试');await page.locator('[data-view=settings]').click();assert.equal(await page.locator('#ar-discard-dialog').isVisible(),true);await page.locator('#ar-discard-cancel').click();assert.equal(await page.locator('#ar-name').inputValue(),'未保存测试');await page.locator('[data-view=settings]').click();await page.locator('#ar-discard-confirm').click();assert.equal(await page.locator('[data-screen=settings]').isVisible(),true);pass('未保存离开、继续编辑和放弃修改');
    await page.locator('[data-view=manager]').click();await page.locator('#ar-filter').fill('new');assert.equal(await page.locator('#ar-table tr').count(),1);await page.locator('[data-edit="11"]').click();await page.locator('#ar-delete').click();assert.equal(await page.locator('#ar-table-count').innerText(),'显示 0 项，共 10 项');await page.getByRole('button',{name:'撤销删除'}).click();assert.equal(await page.locator('#ar-table tr').count(),1);pass('筛选、编辑、删除和撤销');
    await page.locator('#ar-filter-clear').click();await page.screenshot({path:path.join(out,'manager.png'),fullPage:true});
    await page.locator('[data-view=settings]').click();await page.locator('[data-category=appearance]').click();await page.locator('#ar-density').selectOption('24');await page.locator('#ar-show-command').uncheck();await page.locator('#ar-show-hint').uncheck();await page.locator('#ar-show-tools').check();await page.locator('[data-view=launcher]').click();assert.equal(await page.locator('#ar-command').isVisible(),false);assert.equal(await page.locator('#ar-operation-hint').isVisible(),false);assert.equal(await page.locator('#ar-hide').isVisible(),true);assert.equal(await page.locator('#altrun-design').evaluate(e=>getComputedStyle(e).getPropertyValue('--ar-row').trim()),'24px');pass('原版 CMD/提示/工具按钮开关联动');
    await page.locator('[data-view=settings]').click();await page.locator('#ar-density').selectOption('20');await page.locator('#ar-show-command').check();await page.locator('#ar-show-hint').check();await page.locator('#ar-show-tools').uncheck();await page.locator('[data-view=launcher]').click();await q.fill('notepad');
    await page.locator('#ar-theme').selectOption('dark');await page.screenshot({path:path.join(out,'launcher-dark.png'),fullPage:true});pass('明暗主题切换');
    await page.locator('[data-view=migration]').click();await page.locator('#ar-import-disable').uncheck();assert.equal(await page.locator('#ar-import').isDisabled(),true);await page.locator('#ar-import-disable').check();await page.locator('#ar-import').click();assert.match(await page.locator('#ar-status').innerText(),/未扫描或修改/);await page.screenshot({path:path.join(out,'migration.png'),fullPage:true});pass('示例迁移与冲突停用保护');
    const layout=[];
    for(const width of [1024,736,420,320]){
      await page.setViewportSize({width,height:850});
      for(const view of ['launcher','manager','settings','migration']){
        await page.locator(`[data-view=${view}]`).click();
        const dims=await page.evaluate(()=>({w:document.documentElement.clientWidth,scroll:document.documentElement.scrollWidth}));
        assert.ok(dims.scroll<=dims.w+1,`${view} horizontal overflow at ${width}: ${JSON.stringify(dims)}`);layout.push(`${view}@${width}`);
      }
    }
    await page.locator('[data-view=manager]').click();await page.screenshot({path:path.join(out,'narrow-manager.png'),fullPage:true});pass('16 个页面/宽度组合无页面横向溢出');
    assert.deepEqual(errors,[]);pass('无浏览器脚本错误');
    fs.writeFileSync(path.join(out,'prototype-checks.json'),JSON.stringify({date:'2026-11-07',status:'passed',cases,layout,errors,limitations:['仅网页交互原型，未验证 Windows 原生程序','合成输入事件不是实际中文输入法验证','未测试系统热键、磁盘迁移或性能预算']},null,2));
    console.log(JSON.stringify({passed:cases.length,cases,layoutChecks:layout.length,errors},null,2));
  }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
