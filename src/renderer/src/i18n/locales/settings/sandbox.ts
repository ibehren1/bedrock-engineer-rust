export const sandboxSettings = {
  en: {
    sandboxSettings: {
      environment: {
        title: 'Docker',
        description:
          'The sandbox runs each chat in its own Docker container. Docker must be installed and running; Compose is optional and only needed for multi-service sandboxes.'
      },
      image: {
        title: 'Default container image',
        description:
          'The operating system new sandboxes are built on. The agent can still ask for a different one when it creates a sandbox.',
        label: 'Operating system',
        hint: 'Used for new sandboxes. A chat that already has a sandbox keeps its OS until the sandbox is removed.',
        debian: 'Quickest to download and to install packages on. Uses apt-get.',
        ubuntu: 'Uses apt-get.',
        rhel: 'Uses dnf.',
        ubi: 'Real RHEL, limited to Red Hat’s freely available packages. Uses dnf.'
      },
      limits: {
        title: 'Container limits',
        memory: 'Memory limit',
        cpu: 'CPU limit',
        timeout: 'Command timeout'
      },
      toolSummary:
        'Current settings: {{os}}, {{memory}} memory, {{cpu}}, {{timeout}} command timeout.',
      openSettings: 'Open sandbox settings'
    }
  },
  ja: {
    sandboxSettings: {
      environment: {
        title: 'Docker',
        description:
          'サンドボックスはチャットごとに専用の Docker コンテナで動作します。Docker のインストールと起動が必要です。Compose は任意で、複数サービスのサンドボックスにのみ必要です。'
      },
      image: {
        title: '既定のコンテナイメージ',
        description:
          '新しいサンドボックスで使う OS です。エージェントはサンドボックス作成時に別の OS を指定することもできます。',
        label: 'オペレーティングシステム',
        hint: '新しいサンドボックスに適用されます。既にサンドボックスがあるチャットは、削除するまで現在の OS のままです。',
        debian: 'ダウンロードとパッケージのインストールが最も速い OS です。apt-get を使います。',
        ubuntu: 'apt-get を使います。',
        rhel: 'dnf を使います。',
        ubi: '本物の RHEL ですが、Red Hat が無償提供するパッケージのみ利用できます。dnf を使います。'
      },
      limits: {
        title: 'コンテナの制限',
        memory: 'メモリ上限',
        cpu: 'CPU 上限',
        timeout: 'コマンドのタイムアウト'
      },
      toolSummary:
        '現在の設定: {{os}}、メモリ {{memory}}、{{cpu}}、コマンドのタイムアウト {{timeout}}。',
      openSettings: 'サンドボックス設定を開く'
    }
  }
}
