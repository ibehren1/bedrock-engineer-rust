import { withoutRemoteImages } from './webContent'

describe('withoutRemoteImages', () => {
  it('drops remote images and keeps data: ones', () => {
    const html =
      '<p><img src="https://x.example/a.png" alt="chart"/> <img src="data:image/png;base64,AA" alt="d"/> <img alt="" src="http://y/b.png"></p>'
    expect(withoutRemoteImages(html)).toBe(
      '<p><span>[chart]</span> <img src="data:image/png;base64,AA" alt="d"/> </p>'
    )
  })
})
