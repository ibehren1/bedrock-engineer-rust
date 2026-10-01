/** Image generation models the generateImage tool accepts. */
export type ImageGeneratorModel =
  | 'stability.sd3-large-v1:0'
  | 'stability.sd3-5-large-v1:0'
  | 'stability.stable-image-core-v1:0'
  | 'stability.stable-image-core-v1:1'
  | 'stability.stable-image-ultra-v1:0'
  | 'stability.stable-image-ultra-v1:1'
  | 'amazon.nova-canvas-v1:0'
  | 'amazon.titan-image-generator-v2:0'
  | 'amazon.titan-image-generator-v1'

export type AspectRatio = '1:1' | '16:9' | '2:3' | '3:2' | '4:5' | '5:4' | '9:16' | '9:21'
