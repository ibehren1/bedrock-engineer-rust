/* eslint-disable react/prop-types */
import { Modal, ModalBody, ModalFooter, ModalHeader, type ModalProps } from 'flowbite-react'
import { useState } from 'react'

type CustomModalProps = {
  children: React.ReactNode
  header?: React.ReactNode
  footer?: React.ReactNode
  size?: ModalProps['size']
}

/**
 * @deprecated use flowbite-react Modal instead
 * @returns
 */
const useModal = () => {
  const [show, setShow] = useState(false)

  const openModal = () => {
    setShow(true)
  }

  const closeModal = () => {
    setShow(false)
  }

  const CustomModal: React.FC<CustomModalProps> = ({ children, header, footer, size }) => (
    <>
      <Modal dismissible show={show} onClose={() => setShow(false)} size={size}>
        {header && <ModalHeader>{header}</ModalHeader>}
        <ModalBody>{children}</ModalBody>
        {footer && <ModalFooter>{footer}</ModalFooter>}
      </Modal>
    </>
  )

  return {
    Modal: CustomModal,
    openModal,
    closeModal
  }
}

export default useModal
