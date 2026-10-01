import { TbPencil, TbTrash } from 'react-icons/tb'
import { ToolName, isMcpTool } from '@/types/tools'
import {
  FaFolderPlus,
  FaFileSignature,
  FaFileAlt,
  FaList,
  FaArrowRight,
  FaCopy,
  FaSearch,
  FaGlobe,
  FaImage,
  FaDatabase,
  FaTerminal,
  FaBrain,
  FaServer,
  FaCamera,
  FaVideo,
  FaProjectDiagram,
  FaCode,
  FaPlay,
  FaClock,
  FaDownload,
  FaDesktop,
  FaUsers
} from 'react-icons/fa'
import { FaListCheck } from 'react-icons/fa6'
import { FaDocker } from 'react-icons/fa'
import { BiFace } from 'react-icons/bi'
import { MdDifference } from 'react-icons/md'

// 既存のアイコンコンポーネント
export const EditIcon = () => <TbPencil size={18} />
export const RemoveIcon = () => <TbTrash size={18} />

// 標準ツールのアイコン定義
const standardToolIcons = {
  createFolder: <FaFolderPlus className="text-accent size-4" />,
  writeToFile: <FaFileSignature className="text-success size-4" />,
  readFiles: <FaFileAlt className="text-warning size-4" />,
  listFiles: <FaList className="text-purple-500 size-4" />,
  moveFile: <FaArrowRight className="text-warning size-4" />,
  copyFile: <FaCopy className="text-accent size-4" />,
  tavilySearch: <FaSearch className="text-danger size-4" />,
  fetchWebsite: <FaGlobe className="text-success size-4" />,
  generateImage: <FaImage className="text-pink-500 size-4" />,
  generateVideo: <FaPlay className="text-danger size-4" />,
  checkVideoStatus: <FaClock className="text-accent size-4" />,
  downloadVideo: <FaDownload className="text-success size-4" />,
  recognizeImage: <FaCamera className="text-violet-500 size-4" />,
  retrieve: <FaDatabase className="text-success size-4" />,
  invokeBedrockAgent: <BiFace className="text-purple-700 size-4" />,
  executeCommand: <FaTerminal className="text-ink-muted size-4" />,
  applyDiffEdit: <MdDifference className="text-accent size-4" />,
  think: <FaBrain className="text-warning size-4" />,
  invokeFlow: <FaProjectDiagram className="text-accent size-4" />,
  codeInterpreter: <FaCode className="text-success size-4" />,
  dockerSandbox: <FaDocker className="text-[#2496ED] size-4" />,
  screenCapture: <FaDesktop className="text-ink-muted size-4" />,
  cameraCapture: <FaVideo className="text-accent size-4" />,
  todo: <FaListCheck className="text-accent size-4" />,
  todoInit: <FaListCheck className="text-accent size-4" />,
  todoUpdate: <FaListCheck className="text-accent size-4" />,
  invokeAgent: <FaUsers className="text-fuchsia-500 size-4" />
}

// MCPツール用のアイコン（すべてのMCPツールで共通）
const mcpIcon = <FaServer className="text-accent size-4" />

// ツール名に応じて動的にアイコンを返すプロキシ
export const toolIcons = new Proxy({} as { [key in ToolName]: React.ReactElement }, {
  get: (_target, prop: string) => {
    // 標準ツールのアイコンがあればそれを返す
    if (prop in standardToolIcons) {
      return standardToolIcons[prop as keyof typeof standardToolIcons]
    }

    // MCPツールの場合
    if (isMcpTool(prop)) {
      // すべてのMCPツールには同じアイコンを返す
      return mcpIcon
    }

    // 未知のツールの場合もMCPアイコンを返す
    return mcpIcon
  }
})
