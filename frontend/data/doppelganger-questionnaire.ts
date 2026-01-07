// Doppelgänger 问卷题库
// 5个阶段，每阶段包含不同类型的问题

export interface QuestionOption {
  value: string;
  label: string;
}

export interface Question {
  id: string;
  type: 'single' | 'multiple' | 'text' | 'scale' | 'dilemma';
  question: string;
  description?: string;
  options?: QuestionOption[];
  placeholder?: string;
  min?: number;
  max?: number;
  required?: boolean;
}

export interface QuestionnaireLevel {
  id: number;
  name: string;
  description: string;
  questions: Question[];
}

export const questionnaireLevels: QuestionnaireLevel[] = [
  {
    id: 1,
    name: '基础画像',
    description: '让我们先了解一些基本信息',
    questions: [
      {
        id: 'name',
        type: 'text',
        question: '你的名字是？',
        placeholder: '可以是真名或昵称',
        required: true,
      },
      {
        id: 'age',
        type: 'single',
        question: '你的年龄段？',
        options: [
          { value: '18-24', label: '18-24岁' },
          { value: '25-29', label: '25-29岁' },
          { value: '30-34', label: '30-34岁' },
          { value: '35-39', label: '35-39岁' },
          { value: '40+', label: '40岁以上' },
        ],
        required: true,
      },
      {
        id: 'location',
        type: 'text',
        question: '你目前居住在哪个城市？',
        placeholder: '例如：上海',
        required: true,
      },
      {
        id: 'occupation',
        type: 'text',
        question: '你的职业是？',
        placeholder: '例如：软件工程师',
        required: true,
      },
      {
        id: 'mbti',
        type: 'single',
        question: '你的 MBTI 人格类型（如果知道的话）',
        options: [
          { value: 'INTJ', label: 'INTJ - 建筑师' },
          { value: 'INTP', label: 'INTP - 逻辑学家' },
          { value: 'ENTJ', label: 'ENTJ - 指挥官' },
          { value: 'ENTP', label: 'ENTP - 辩论家' },
          { value: 'INFJ', label: 'INFJ - 提倡者' },
          { value: 'INFP', label: 'INFP - 调停者' },
          { value: 'ENFJ', label: 'ENFJ - 主人公' },
          { value: 'ENFP', label: 'ENFP - 竞选者' },
          { value: 'ISTJ', label: 'ISTJ - 物流师' },
          { value: 'ISFJ', label: 'ISFJ - 守卫者' },
          { value: 'ESTJ', label: 'ESTJ - 总经理' },
          { value: 'ESFJ', label: 'ESFJ - 执政官' },
          { value: 'ISTP', label: 'ISTP - 鉴赏家' },
          { value: 'ISFP', label: 'ISFP - 探险家' },
          { value: 'ESTP', label: 'ESTP - 企业家' },
          { value: 'ESFP', label: 'ESFP - 表演者' },
          { value: 'unknown', label: '不确定/没测过' },
        ],
      },
      {
        id: 'languages',
        type: 'multiple',
        question: '你会说哪些语言？',
        options: [
          { value: 'mandarin', label: '普通话' },
          { value: 'cantonese', label: '粤语' },
          { value: 'english', label: '英语' },
          { value: 'japanese', label: '日语' },
          { value: 'other', label: '其他' },
        ],
        required: true,
      },
    ],
  },
  {
    id: 2,
    name: '价值观探测',
    description: '通过选择题了解你的价值取向',
    questions: [
      {
        id: 'career_money_vs_time',
        type: 'dilemma',
        question: '两个工作机会，你会如何选择？',
        description: 'A: 薪资+30%，但需要996工作制\nB: 薪资不变，965正常作息',
        options: [
          { value: 'A', label: '选A - 先赚钱，吃苦是投资' },
          { value: 'B', label: '选B - 生活质量和健康更重要' },
          { value: 'depends', label: '需要更多信息才能决定' },
        ],
        required: true,
      },
      {
        id: 'housing_stance',
        type: 'single',
        question: '对于买房，你的态度是？',
        options: [
          { value: 'must_buy', label: '必须买，是安全感/资产配置' },
          { value: 'wait', label: '等待时机，目前不是好时候' },
          { value: 'never', label: '不考虑，租房也可以' },
          { value: 'already', label: '已经有房' },
        ],
        required: true,
      },
      {
        id: 'risk_tolerance',
        type: 'scale',
        question: '你的风险承受能力？',
        description: '1 = 极度保守（只接受银行存款）\n10 = 极度激进（愿意All-in高风险投资）',
        min: 1,
        max: 10,
        required: true,
      },
      {
        id: 'success_definition',
        type: 'single',
        question: '你认为"成功"最重要的是？',
        options: [
          { value: 'wealth', label: '财务自由' },
          { value: 'status', label: '社会地位/影响力' },
          { value: 'family', label: '家庭幸福' },
          { value: 'freedom', label: '自由/按自己方式生活' },
          { value: 'achievement', label: '专业成就/做出有意义的事' },
        ],
        required: true,
      },
      {
        id: 'comfort_vs_growth',
        type: 'dilemma',
        question: '舒适区 vs 成长区',
        description: '现在有一个机会，能让你学到很多但会非常辛苦，你会？',
        options: [
          { value: 'take', label: '接受，成长比舒适重要' },
          { value: 'evaluate', label: '看具体ROI再决定' },
          { value: 'decline', label: '拒绝，当前状态挺好' },
        ],
        required: true,
      },
    ],
  },
  {
    id: 3,
    name: '语言风格',
    description: '帮助 AI 学习你的表达方式',
    questions: [
      {
        id: 'chat_style',
        type: 'text',
        question: '用你平时发微信的语气，描述一下今天的心情',
        placeholder: '随便写，越真实越好...',
        required: true,
      },
      {
        id: 'angry_response',
        type: 'text',
        question: '假设有人做了让你很不爽的事，你会怎么表达？',
        description: '可以是发微信/朋友圈/或者你会做什么',
        placeholder: '例如：直接怼回去 / 冷处理 / 发朋友圈阴阳...',
        required: true,
      },
      {
        id: 'emoji_usage',
        type: 'single',
        question: '你发消息时使用表情包的频率？',
        options: [
          { value: 'never', label: '几乎不用' },
          { value: 'low', label: '偶尔用' },
          { value: 'medium', label: '经常用' },
          { value: 'high', label: '每条消息都有' },
        ],
        required: true,
      },
      {
        id: 'catchphrases',
        type: 'text',
        question: '你有什么口头禅或常用的表达？',
        placeholder: '例如：绝绝子/破防了/无语住了...',
      },
      {
        id: 'communication_preference',
        type: 'single',
        question: '你更喜欢哪种沟通方式？',
        options: [
          { value: 'direct', label: '直接明了，不喜欢绕弯子' },
          { value: 'gentle', label: '委婉含蓄，照顾对方感受' },
          { value: 'humorous', label: '幽默调侃，喜欢用梗' },
          { value: 'formal', label: '正式严谨，注重措辞' },
        ],
        required: true,
      },
    ],
  },
  {
    id: 4,
    name: '决策回溯',
    description: '回忆一些重要的人生决策',
    questions: [
      {
        id: 'important_decision',
        type: 'text',
        question: '回忆一个你犹豫了很久的重要决定，最后怎么选的？为什么？',
        placeholder: '例如：换工作、分手、搬家...',
        required: true,
      },
      {
        id: 'regret_decision',
        type: 'text',
        question: '有没有什么决定是你后悔的？',
        placeholder: '如果没有可以写"目前没有"',
      },
      {
        id: 'proud_decision',
        type: 'text',
        question: '有没有什么决定是你特别自豪的？',
        placeholder: '写一个让你觉得当时做对了的决定',
        required: true,
      },
      {
        id: 'decision_speed',
        type: 'single',
        question: '你做重要决定通常需要多久？',
        options: [
          { value: 'instant', label: '很快，相信直觉' },
          { value: 'days', label: '几天，需要仔细考虑' },
          { value: 'weeks', label: '几周，需要收集很多信息' },
          { value: 'struggle', label: '很难决定，经常纠结很久' },
        ],
        required: true,
      },
    ],
  },
  {
    id: 5,
    name: '深度画像',
    description: '最后一些深度问题',
    questions: [
      {
        id: 'triggers_negative',
        type: 'text',
        question: '什么事情会让你感到不爽或焦虑？',
        placeholder: '例如：被催婚/加班/微管理...',
        required: true,
      },
      {
        id: 'triggers_positive',
        type: 'text',
        question: '什么事情会让你感到特别开心或满足？',
        placeholder: '例如：赚到钱/得到认可/完成目标...',
        required: true,
      },
      {
        id: 'controversial_work',
        type: 'single',
        question: '对于"996是福报"这种观点，你的态度是？',
        options: [
          { value: 'agree', label: '有一定道理，努力才能成功' },
          { value: 'disagree', label: '完全不同意，这是剥削' },
          { value: 'context', label: '看情况，要看回报是否匹配' },
        ],
        required: true,
      },
      {
        id: 'life_philosophy',
        type: 'text',
        question: '用一句话描述你的人生哲学或座右铭',
        placeholder: '例如：延迟满足 / 活在当下 / 追求卓越...',
        required: true,
      },
      {
        id: 'future_self',
        type: 'text',
        question: '5年后的你，理想状态是什么样的？',
        placeholder: '描述一下你期望的生活/工作/财务状态',
        required: true,
      },
      {
        id: 'aesthetic_preference',
        type: 'multiple',
        question: '你对哪些类型的内容/风格更感兴趣？',
        options: [
          { value: 'villain', label: '反派/反英雄角色' },
          { value: 'strategy', label: '高智商博弈/策略' },
          { value: 'aesthetic', label: '极致美学/视觉艺术' },
          { value: 'tech', label: '前沿科技/编程' },
          { value: 'finance', label: '投资理财' },
          { value: 'philosophy', label: '哲学/人生思考' },
        ],
      },
    ],
  },
];
