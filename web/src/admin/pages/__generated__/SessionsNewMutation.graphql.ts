/**
 * @generated SignedSource<<f4820e0468f5fe83d7548c8318b39d0b>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SessionsNewMutation$variables = {
  config?: string | null | undefined;
  healthcheckUrl?: string | null | undefined;
  locationId: string;
  name: string;
};
export type SessionsNewMutation$data = {
  readonly createSession: {
    readonly code: string | null | undefined;
    readonly id: string;
  };
};
export type SessionsNewMutation = {
  response: SessionsNewMutation$data;
  variables: SessionsNewMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "config"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "healthcheckUrl"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "locationId"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "name"
},
v4 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "config",
        "variableName": "config"
      },
      {
        "kind": "Variable",
        "name": "healthcheckUrl",
        "variableName": "healthcheckUrl"
      },
      {
        "kind": "Variable",
        "name": "locationId",
        "variableName": "locationId"
      },
      {
        "kind": "Variable",
        "name": "name",
        "variableName": "name"
      }
    ],
    "concreteType": "Session",
    "kind": "LinkedField",
    "name": "createSession",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "code",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*:: as any*/),
      (v1/*:: as any*/),
      (v2/*:: as any*/),
      (v3/*:: as any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "SessionsNewMutation",
    "selections": (v4/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v3/*:: as any*/),
      (v2/*:: as any*/),
      (v0/*:: as any*/),
      (v1/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "SessionsNewMutation",
    "selections": (v4/*:: as any*/)
  },
  "params": {
    "cacheID": "3c8c39b8dbea87d6bcad42ff26342997",
    "id": null,
    "metadata": {},
    "name": "SessionsNewMutation",
    "operationKind": "mutation",
    "text": "mutation SessionsNewMutation(\n  $name: String!\n  $locationId: ID!\n  $config: String\n  $healthcheckUrl: String\n) {\n  createSession(name: $name, locationId: $locationId, config: $config, healthcheckUrl: $healthcheckUrl) {\n    id\n    code\n  }\n}\n"
  }
};
})();

(node as any).hash = "5516e0d85be62d5842bb07e54d4ce593";

export default node;
