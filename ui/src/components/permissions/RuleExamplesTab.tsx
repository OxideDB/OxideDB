import React, { useState } from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Code, Copy, Check, Lightbulb, BookOpen } from 'lucide-react';
import { ruleExamples, availableVariables } from '@/utils/permissions/constants';

export const RuleExamplesTab: React.FC = () => {
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);

  const copyToClipboard = async (text: string, index: number) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedIndex(index);
      setTimeout(() => setCopiedIndex(null), 2000);
    } catch (err) {
      console.error('Failed to copy text: ', err);
    }
  };

  return (
    <div className="space-y-6">
      {/* Simplified intro */}
      <div className="space-y-1">
        <p className="text-muted-foreground leading-relaxed">
          Ready-to-use access rule patterns for common scenarios and reference guide for available variables. Use these examples as starting points for your own custom rules.
        </p>
      </div>

      {/* Rule Examples Section */}
      <Card className="border-2 shadow-sm">
        <CardHeader className="pb-6 border-b border-border/50">
          <div className="flex items-center justify-between">
            <div className="space-y-1">
              <CardTitle className="flex items-center gap-2 text-lg">
                <Code className="h-4 w-4 text-primary" />
                Rule Examples
              </CardTitle>
              <CardDescription>
                Ready-to-use access rule patterns for common scenarios. Click "Copy Rule" to copy the expression to your clipboard.
              </CardDescription>
            </div>
            <div className="text-sm text-muted-foreground">
              {ruleExamples.length} examples
            </div>
          </div>
        </CardHeader>
        <CardContent className="p-6 lg:p-8">
          <div className="grid gap-6 grid-cols-1 xl:grid-cols-2">
            {ruleExamples.map((example, index) => (
              <Card key={index} className="border-2 transition-all duration-200 hover:shadow-lg hover:border-primary/30 group">
                <CardContent className="p-6">
                  <div className="space-y-5">
                    <div className="space-y-3">
                      <div className="flex items-start justify-between">
                        <h4 className="font-bold text-lg leading-tight pr-4">{example.title}</h4>
                        <div className="flex-shrink-0">
                          <Button 
                            variant="outline" 
                            size="sm"
                            onClick={() => copyToClipboard(example.rule, index)}
                            className="opacity-0 group-hover:opacity-100 transition-opacity duration-200"
                          >
                            {copiedIndex === index ? (
                              <Check className="w-4 h-4 text-green-600" />
                            ) : (
                              <Copy className="w-4 h-4" />
                            )}
                          </Button>
                        </div>
                      </div>
                      <p className="text-muted-foreground leading-relaxed">{example.description}</p>
                    </div>
                    
                    <div className="space-y-3">
                      <label className="text-sm font-semibold text-foreground flex items-center gap-2">
                        <div className="w-2 h-2 bg-green-500 rounded-full"></div>
                        Rule Expression
                      </label>
                      <div className="relative">
                        <div className="font-mono text-sm bg-gradient-to-r from-muted/50 to-muted/70 p-4 rounded-lg border-2 break-all leading-relaxed">
                          {example.rule}
                        </div>
                        <Button 
                          variant="ghost" 
                          size="sm"
                          onClick={() => copyToClipboard(example.rule, index)}
                          className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity duration-200 h-8 w-8 p-0"
                        >
                          {copiedIndex === index ? (
                            <Check className="w-3 h-3 text-green-600" />
                          ) : (
                            <Copy className="w-3 h-3" />
                          )}
                        </Button>
                      </div>
                    </div>
                    
                    <Button 
                      variant="outline" 
                      className="w-full font-medium" 
                      size="sm"
                      onClick={() => copyToClipboard(example.rule, index)}
                    >
                      {copiedIndex === index ? (
                        <>
                          <Check className="w-4 h-4 mr-2 text-green-600" />
                          <span className="text-green-600">Copied!</span>
                        </>
                      ) : (
                        <>
                          <Copy className="w-4 h-4 mr-2" />
                          <span>Copy Rule</span>
                        </>
                      )}
                    </Button>
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Available Variables Section */}
      <Card className="border-2 shadow-sm">
        <CardHeader className="pb-6 border-b border-border/50">
          <div className="flex items-center justify-between">
            <div className="space-y-1">
              <CardTitle className="flex items-center gap-2 text-lg">
                <BookOpen className="h-4 w-4 text-primary" />
                Available Variables
              </CardTitle>
              <CardDescription>
                Reference guide for variables you can use in your access rules. These provide context about the request, user, and data.
              </CardDescription>
            </div>
            <div className="text-sm text-muted-foreground">
              Quick Reference
            </div>
          </div>
        </CardHeader>
        <CardContent className="p-6 lg:p-8">
          <div className="space-y-10">
            {/* Request Variables */}
            <div className="space-y-6">
              <div className="flex items-center justify-between">
                <div className="flex items-center space-x-4">
                  <div className="w-12 h-12 bg-gradient-to-br from-blue-100 to-blue-200 rounded-xl flex items-center justify-center shadow-sm">
                    <span className="text-blue-700 font-bold text-lg">R</span>
                  </div>
                  <div>
                    <h4 className="font-semibold text-base">Request Variables</h4>
                    <p className="text-sm text-muted-foreground mt-1">Access information about the incoming request</p>
                  </div>
                </div>
                <div className="text-sm text-muted-foreground">
                  {availableVariables.request.length} variables
                </div>
              </div>
              
              <div className="grid gap-4 lg:grid-cols-2">
                {availableVariables.request.map((item, index) => (
                  <div key={index} className="group">
                    <div className="flex flex-col lg:flex-row lg:items-center gap-4 p-4 rounded-xl border-2 bg-gradient-to-r from-blue-50/30 to-blue-50/50 hover:border-blue-300 transition-all duration-200">
                      <code className="bg-blue-100 text-blue-800 px-3 py-2 rounded-lg font-mono text-sm border border-blue-200 flex-shrink-0 font-medium">
                        {item.variable}
                      </code>
                      <div className="min-w-0 flex-1">
                        <span className="text-muted-foreground leading-relaxed">{item.description}</span>
                      </div>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => copyToClipboard(item.variable, -1)}
                        className="opacity-0 group-hover:opacity-100 transition-opacity duration-200 flex-shrink-0 h-8 w-8 p-0"
                      >
                        <Copy className="w-3 h-3" />
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            </div>
            
            {/* Record Variables */}
            <div className="space-y-6">
              <div className="flex items-center justify-between">
                <div className="flex items-center space-x-4">
                  <div className="w-12 h-12 bg-gradient-to-br from-green-100 to-green-200 rounded-xl flex items-center justify-center shadow-sm">
                    <span className="text-green-700 font-bold text-lg">D</span>
                  </div>
                  <div>
                    <h4 className="font-semibold text-base">Record Variables</h4>
                    <p className="text-sm text-muted-foreground mt-1">Access data from the record being operated on</p>
                  </div>
                </div>
                <div className="text-sm text-muted-foreground">
                  {availableVariables.record.length} variables
                </div>
              </div>
              
              <div className="grid gap-4 lg:grid-cols-2">
                {availableVariables.record.map((item, index) => (
                  <div key={index} className="group">
                    <div className="flex flex-col lg:flex-row lg:items-center gap-4 p-4 rounded-xl border-2 bg-gradient-to-r from-green-50/30 to-green-50/50 hover:border-green-300 transition-all duration-200">
                      <code className="bg-green-100 text-green-800 px-3 py-2 rounded-lg font-mono text-sm border border-green-200 flex-shrink-0 font-medium">
                        {item.variable}
                      </code>
                      <div className="min-w-0 flex-1">
                        <span className="text-muted-foreground leading-relaxed">{item.description}</span>
                      </div>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => copyToClipboard(item.variable, -1)}
                        className="opacity-0 group-hover:opacity-100 transition-opacity duration-200 flex-shrink-0 h-8 w-8 p-0"
                      >
                        <Copy className="w-3 h-3" />
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          
            {/* System Variables */}
            <div className="space-y-6">
              <div className="flex items-center justify-between">
                <div className="flex items-center space-x-4">
                  <div className="w-12 h-12 bg-gradient-to-br from-purple-100 to-purple-200 rounded-xl flex items-center justify-center shadow-sm">
                    <span className="text-purple-700 font-bold text-lg">S</span>
                  </div>
                  <div>
                    <h4 className="font-semibold text-base">System Variables</h4>
                    <p className="text-sm text-muted-foreground mt-1">Access system-level information like current time</p>
                  </div>
                </div>
                <div className="text-sm text-muted-foreground">
                  {availableVariables.system.length} variables
                </div>
              </div>
              
              <div className="grid gap-4 lg:grid-cols-2">
                {availableVariables.system.map((item, index) => (
                  <div key={index} className="group">
                    <div className="flex flex-col lg:flex-row lg:items-center gap-4 p-4 rounded-xl border-2 bg-gradient-to-r from-purple-50/30 to-purple-50/50 hover:border-purple-300 transition-all duration-200">
                      <code className="bg-purple-100 text-purple-800 px-3 py-2 rounded-lg font-mono text-sm border border-purple-200 flex-shrink-0 font-medium">
                        {item.variable}
                      </code>
                      <div className="min-w-0 flex-1">
                        <span className="text-muted-foreground leading-relaxed">{item.description}</span>
                      </div>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => copyToClipboard(item.variable, -1)}
                        className="opacity-0 group-hover:opacity-100 transition-opacity duration-200 flex-shrink-0 h-8 w-8 p-0"
                      >
                        <Copy className="w-3 h-3" />
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>

          {/* Quick Tips Section */}
          <div className="mt-8 p-4 bg-gradient-to-r from-amber-50 to-orange-50 rounded-lg border border-amber-200">
            <div className="flex items-start gap-3">
              <div className="p-1.5 bg-amber-100 rounded-md flex-shrink-0">
                <Lightbulb className="h-4 w-4 text-amber-600" />
              </div>
              <div className="space-y-2">
                <h4 className="font-semibold text-base text-amber-900">Quick Tips</h4>
                <div className="space-y-1.5 text-sm text-amber-800 leading-relaxed">
                  <p>• Combine variables with operators like <code className="bg-amber-100 px-1 py-0.5 rounded text-xs">=</code>, <code className="bg-amber-100 px-1 py-0.5 rounded text-xs">!=</code>, <code className="bg-amber-100 px-1 py-0.5 rounded text-xs">&gt;</code>, <code className="bg-amber-100 px-1 py-0.5 rounded text-xs">&lt;</code> to create conditions</p>
                  <p>• Use <code className="bg-amber-100 px-1 py-0.5 rounded text-xs">&&</code> and <code className="bg-amber-100 px-1 py-0.5 rounded text-xs">||</code> to combine multiple conditions</p>
                  <p>• String values should be wrapped in quotes: <code className="bg-amber-100 px-1 py-0.5 rounded text-xs">@record.status = 'active'</code></p>
                  <p>• Use <code className="bg-amber-100 px-1 py-0.5 rounded text-xs">~</code> for pattern matching with wildcards</p>
                  <p>• Test your rules thoroughly to ensure they work as expected</p>
                </div>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
};
