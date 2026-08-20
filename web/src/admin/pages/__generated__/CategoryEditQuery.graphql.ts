/**
 * @generated SignedSource<<ee493e42e4121a1423608ca425b76e3c>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type CategoryEditQuery$variables = {
  id: string;
};
export type CategoryEditQuery$data = {
  readonly category: {
    readonly enabled: boolean;
    readonly id: string;
    readonly isVirtual: boolean;
    readonly name: string;
    readonly nitcGroupId: string | null | undefined;
    readonly nitcParticipantType: string | null | undefined;
  };
  readonly nitcGroups: ReadonlyArray<{
    readonly id: string;
    readonly nitcType: string;
  }>;
  readonly ses_participant_types: ReadonlyArray<string>;
};
export type CategoryEditQuery = {
  response: CategoryEditQuery$data;
  variables: CategoryEditQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "id"
  }
],
v1 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v2 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      }
    ],
    "concreteType": "Category",
    "kind": "LinkedField",
    "name": "category",
    "plural": false,
    "selections": [
      (v1/*: any*/),
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "name",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "enabled",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "isVirtual",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "nitcGroupId",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "nitcParticipantType",
        "storageKey": null
      }
    ],
    "storageKey": null
  },
  {
    "alias": null,
    "args": null,
    "concreteType": "NitcGroup",
    "kind": "LinkedField",
    "name": "nitcGroups",
    "plural": true,
    "selections": [
      (v1/*: any*/),
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "nitcType",
        "storageKey": null
      }
    ],
    "storageKey": null
  },
  {
    "alias": null,
    "args": null,
    "kind": "ScalarField",
    "name": "ses_participant_types",
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "CategoryEditQuery",
    "selections": (v2/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "CategoryEditQuery",
    "selections": (v2/*: any*/)
  },
  "params": {
    "cacheID": "b565fa6d2cc107ec05a9028a04135ed7",
    "id": null,
    "metadata": {},
    "name": "CategoryEditQuery",
    "operationKind": "query",
    "text": "query CategoryEditQuery(\n  $id: ID!\n) {\n  category(id: $id) {\n    id\n    name\n    enabled\n    isVirtual\n    nitcGroupId\n    nitcParticipantType\n  }\n  nitcGroups {\n    id\n    nitcType\n  }\n  ses_participant_types\n}\n"
  }
};
})();

(node as any).hash = "73e3f244025378c5a6f8a61b6faf9065";

export default node;
