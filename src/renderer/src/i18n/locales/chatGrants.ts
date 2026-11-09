export const chatGrants = {
  en: {
    chatGrants: {
      buttonTooltip: '{{title}}: {{count}} on',
      agents: {
        title: 'Agents for this chat',
        help: 'An agent that is on can be handed work for the rest of this chat. Type @Name in a message to turn one on, or disable @Name to turn it off.',
        noInvokeAgent:
          "This chat's agent doesn't have the invokeAgent tool, so it can't hand work to these agents. Turn it on in the agent's Tools tab."
      },
      knowledgeBases: {
        title: 'Knowledge bases for this chat',
        help: 'While any knowledge base is on, the agent searches only the ones that are on, for the rest of this chat. Type @Name in a message to turn one on, or disable @Name to turn it off.'
      }
    }
  },
  ja: {
    chatGrants: {
      buttonTooltip: '{{title}}: {{count}} 件オン',
      agents: {
        title: 'このチャットのエージェント',
        help: 'オンのエージェントには、このチャットの間ずっと作業を委譲できます。メッセージに @名前 と書くとオン、disable @名前 と書くとオフになります。',
        noInvokeAgent:
          'このチャットのエージェントには invokeAgent ツールがないため、これらのエージェントに委譲できません。エージェントの「ツール」タブでオンにしてください。'
      },
      knowledgeBases: {
        title: 'このチャットのナレッジベース',
        help: 'ナレッジベースが 1 つでもオンの間、エージェントはこのチャットの間ずっとオンのものだけを検索します。メッセージに @名前 と書くとオン、disable @名前 と書くとオフになります。'
      }
    }
  }
}
