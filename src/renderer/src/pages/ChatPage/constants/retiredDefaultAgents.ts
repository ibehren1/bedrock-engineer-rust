import { CustomAgent } from '@/types/agent-chat'

/**
 * Agents that shipped as built-in defaults and later moved to the Agent Directory. A copy already
 * seeded into `customAgents` is no longer refreshed by the default-agent seeding, so it becomes the
 * user's own (editable) agent instead. Its description and scenario prompts pointed at translation
 * keys that no longer exist, so they are replaced by the text they resolved to.
 */
const RETIRED_DEFAULT_AGENTS: Record<string, Pick<CustomAgent, 'description' | 'scenarios'>> = {
  // Built-in on 2026-10-24, in the directory as aws-solutions-architect.yaml since 2026-10-25.
  awsSolutionsArchitectAgent: {
    description:
      'Answers AWS architecture questions from the official AWS documentation, with links to the sources, and validates CDK / CloudFormation and estimates costs.',
    scenarios: [
      {
        title: 'Compare AWS services',
        content:
          'Compare Amazon SQS, Amazon SNS and Amazon EventBridge for decoupling microservices: delivery guarantees, ordering, throughput limits, pricing model and when to choose each. Link every point to the AWS documentation.'
      },
      {
        title: 'Design a reference architecture',
        content:
          'Design a highly available, multi-AZ architecture for a containerized web application with a relational database on AWS. Explain each service choice against the Well-Architected Framework, draw it as a Mermaid diagram and cite the AWS documentation.'
      },
      {
        title: 'Estimate monthly cost',
        content:
          'Estimate the monthly cost in us-east-1 of an application with an Application Load Balancer, two m7g.large EC2 instances, a db.r7g.large Multi-AZ Aurora PostgreSQL cluster and 500 GB of S3 Standard storage. Show the prices you used and their source.'
      },
      {
        title: 'Generate CDK for a pattern',
        content:
          'Write an AWS CDK (TypeScript) stack for an S3 bucket that triggers a Lambda function writing to DynamoDB, following CDK best practices, and save it to {{projectPath}}. Explain the security settings you chose with links to the documentation.'
      }
    ]
  }
}

/** Turns saved copies of retired default agents into custom agents. */
export function adoptRetiredDefaultAgents(saved: CustomAgent[]): {
  agents: CustomAgent[]
  changed: boolean
} {
  let changed = false
  const agents = saved.map((agent) => {
    const retired = RETIRED_DEFAULT_AGENTS[agent.id]
    if (!retired || agent.isCustom) return agent
    changed = true
    return { ...agent, ...retired, isCustom: true }
  })
  return { agents, changed }
}
