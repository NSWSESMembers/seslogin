/**
 * @generated SignedSource<<a34277967481b06c9207d96f1598c7db>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type KioskRealtimeTokenQuery$variables = Record<PropertyKey, never>;
export type KioskRealtimeTokenQuery$data = {
  readonly kioskRealtimeToken: {
    readonly channel: string;
    readonly tokenRequest: {
      readonly capability: string;
      readonly clientId: string;
      readonly keyName: string;
      readonly mac: string;
      readonly nonce: string;
      readonly timestamp: number;
      readonly ttl: number;
    };
  } | null | undefined;
};
export type KioskRealtimeTokenQuery = {
  response: KioskRealtimeTokenQuery$data;
  variables: KioskRealtimeTokenQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "alias": null,
    "args": null,
    "concreteType": "KioskRealtimeToken",
    "kind": "LinkedField",
    "name": "kioskRealtimeToken",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "channel",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "AblyTokenRequest",
        "kind": "LinkedField",
        "name": "tokenRequest",
        "plural": false,
        "selections": [
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "keyName",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "ttl",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "capability",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "clientId",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "timestamp",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "nonce",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "mac",
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [],
    "kind": "Fragment",
    "metadata": null,
    "name": "KioskRealtimeTokenQuery",
    "selections": (v0/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "KioskRealtimeTokenQuery",
    "selections": (v0/*: any*/)
  },
  "params": {
    "cacheID": "2126a04a810a661f34bdfc3c98f1b009",
    "id": null,
    "metadata": {},
    "name": "KioskRealtimeTokenQuery",
    "operationKind": "query",
    "text": "query KioskRealtimeTokenQuery {\n  kioskRealtimeToken {\n    channel\n    tokenRequest {\n      keyName\n      ttl\n      capability\n      clientId\n      timestamp\n      nonce\n      mac\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "414eb597216cd226ffcdad023d06ddd4";

export default node;
